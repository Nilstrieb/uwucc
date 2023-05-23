mod builder;

use std::cell::RefCell;

use parser::{
    ast::{self, Atom, DeclAttr, Expr, ExternalDecl, Stmt, TranslationUnit, TypeSpecifier},
    Span, Symbol,
};
use rustc_hash::{FxHashMap, FxHashSet};

use self::builder::FuncBuilder;
use crate::{
    ir::{BinKind, Branch, ConstValue, Func, Ir, Layout, Operand, Register, TyLayout},
    ty::{Ty, TyKind},
    Error,
};

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug)]
struct LoweringCx<'cx> {
    tys: RefCell<FxHashSet<&'cx TyKind<'cx>>>,
    layouts: RefCell<FxHashSet<&'cx Layout>>,
    arena: &'cx bumpalo::Bump,
}

impl<'cx> LoweringCx<'cx> {
    fn lower_ty(&self, ty: &TypeSpecifier) -> Ty<'cx> {
        let kind = match ty {
            TypeSpecifier::Void => TyKind::Void,
            TypeSpecifier::Char => TyKind::Char,
            TypeSpecifier::SChar => TyKind::SChar,
            TypeSpecifier::UChar => TyKind::UChar,
            TypeSpecifier::Integer(int) => TyKind::Integer(*int),
            TypeSpecifier::Float => TyKind::Float,
            TypeSpecifier::Double => TyKind::Double,
            TypeSpecifier::LongDouble => TyKind::LongDouble,
            TypeSpecifier::Bool => TyKind::Bool,
        };
        self.intern_ty(kind)
    }

    fn intern_ty(&self, kind: TyKind<'cx>) -> Ty<'cx> {
        let opt_kind = self.tys.borrow().get(&kind).copied();
        match opt_kind {
            Some(ty) => Ty::new_unchecked(ty),
            None => {
                let kind = self.arena.alloc(kind);
                self.tys.borrow_mut().insert(kind);
                Ty::new_unchecked(kind)
            }
        }
    }

    fn intern_layout(&self, layout: Layout) -> &'cx Layout {
        let opt_layout = self.layouts.borrow().get(&layout).copied();
        match opt_layout {
            Some(layout) => layout,
            None => {
                let layout = self.arena.alloc(layout);
                self.layouts.borrow_mut().insert(layout);
                layout
            }
        }
    }

    fn layout_of(&self, ty: Ty<'cx>) -> TyLayout<'cx> {
        let layout = match *ty {
            TyKind::Void => Layout::size_align(0, 1),
            TyKind::Char => Layout::size_align(1, 1),
            TyKind::SChar => Layout::size_align(1, 1),
            TyKind::UChar => Layout::size_align(1, 1),
            TyKind::Integer(int) => match int.kind {
                parser::ast::IntTyKind::Short => Layout::size_align(2, 2),
                parser::ast::IntTyKind::Int => Layout::size_align(4, 4),
                parser::ast::IntTyKind::Long => Layout::size_align(8, 8),
                parser::ast::IntTyKind::LongLong => Layout::size_align(8, 8),
            },
            TyKind::Float => Layout::size_align(4, 4),
            TyKind::Double => Layout::size_align(8, 8),
            TyKind::LongDouble => Layout::size_align(8, 8),
            TyKind::Bool => Layout::size_align(1, 1),
            TyKind::Struct(_) => todo!("layout_of struct"),
            TyKind::Union(_) => todo!("layout_of union"),
            TyKind::Enum(_) => todo!("layout_of enum"),
            TyKind::Ptr(_) => Layout::size_align(8, 8),
        };
        let layout = self.intern_layout(layout);
        TyLayout { ty, layout }
    }
}

pub fn lower_translation_unit<'cx>(
    arena: &'cx bumpalo::Bump,
    ast: &TranslationUnit,
) -> Result<Ir<'cx>, Error> {
    let mut lcx = LoweringCx {
        tys: RefCell::default(),
        layouts: RefCell::default(),
        arena,
    };

    for (decl, _) in ast {
        match decl {
            ExternalDecl::Decl(_) => todo!("decl is unsupported"),
            ExternalDecl::FunctionDef(def) => {
                let decl = def.decl.uwnrap_normal();
                let body = &def.body;
                let ret_ty = lcx.lower_ty(&decl.decl_spec.ty);
                lower_body(
                    &mut lcx,
                    body,
                    decl.init_declarators[0].1,
                    decl.init_declarators[0].0.declarator.decl.name().0,
                    ret_ty,
                )?;
            }
        }
    }

    todo!("building is not really")
}

#[derive(Debug)]
struct FnLoweringCtxt<'a, 'cx> {
    scopes: Vec<FxHashMap<Symbol, VariableInfo<'cx>>>,
    build: FuncBuilder<'a, 'cx>,
    lcx: &'a LoweringCx<'cx>,
}

impl<'a, 'cx> FnLoweringCtxt<'a, 'cx> {
    fn resolve_ident(&self, ident: Symbol) -> Option<&VariableInfo<'cx>> {
        self.scopes.iter().rev().find_map(|s| s.get(&ident))
    }

    fn lower_body(&mut self, body: &[(Stmt, Span)]) -> Result<()> {
        for (stmt, stmt_span) in body {
            self.lower_stmt(stmt, *stmt_span)?;
        }
        Ok(())
    }

    fn lower_stmt(&mut self, stmt: &ast::Stmt, stmt_span: Span) -> Result<()> {
        match stmt {
            Stmt::Decl(decl) => {
                let decl = decl.uwnrap_normal();
                let ty = self.lcx.lower_ty(&decl.decl_spec.ty);
                let decl_attr = decl.decl_spec.attrs;

                for (var, def_span) in &decl.init_declarators {
                    let tyl = self.lcx.layout_of(ty.clone());
                    let (name, _) = var.declarator.decl.name();
                    let ptr_to = self.build.alloca(&tyl.layout, Some(name), stmt_span);

                    let variable_info = VariableInfo {
                        def_span: *def_span,
                        ptr_to,
                        decl_attr,
                        tyl: tyl.clone(),
                    };
                    self.scopes.last_mut().unwrap().insert(name, variable_info);
                    if let Some((init, init_span)) = &var.init {
                        let init = self.lower_expr(init, *init_span)?;
                        self.build.store(ptr_to, init, tyl.layout, *init_span);
                    }
                }
            }
            Stmt::Labeled { .. } => todo!("labels are not implemented"),
            Stmt::Compound(_) => todo!("blocks are not implemented"),
            Stmt::If {
                cond,
                then: then_body,
                otherwise,
            } => {
                let cond = self.lower_expr(&cond.0, cond.1)?;
                let pred = self.build.current_bb;
                let then = self.build.new_block();
                let els = otherwise
                    .as_deref()
                    .map(|oth| (oth, self.build.new_block()));
                let cont = self.build.new_block();

                self.build.current_bb = then;
                self.lower_body(&then_body)?;
                self.build.cur_bb_mut().term = Branch::Goto(cont);

                let false_branch = match els {
                    Some((otherwise, els)) => {
                        self.build.current_bb = els;
                        self.lower_body(&otherwise)?;
                        self.build.cur_bb_mut().term = Branch::Goto(cont);
                        els
                    }
                    None => cont,
                };
                self.build.bb_mut(pred).term = Branch::Switch {
                    cond,
                    yes: then,
                    no: false_branch,
                };
                self.build.current_bb = cont;
            }
            Stmt::Switch => todo!(),
            Stmt::While { cond, body } => todo!(),
            Stmt::For {
                init_decl,
                init_expr,
                cond,
                post,
                body,
            } => todo!(),
            Stmt::Goto(_) => todo!(),
            Stmt::Continue => todo!(),
            Stmt::Break => todo!(),
            Stmt::Return(_) => todo!(),
            Stmt::Expr(ast::Expr::Binary(ast::ExprBinary {
                op: ast::BinaryOp::Assign(assign),
                lhs,
                rhs,
            })) => {
                if assign.is_some() {
                    todo!("assign operation");
                }
                let rhs = self.lower_expr(&rhs.0, rhs.1)?;
                let (Expr::Atom(ast::Atom::Ident((ident, ident_span))), _) = **lhs else {
                    todo!("complex assignments")
                };
                let Some(var) = self.resolve_ident(ident) else {
                    return Err(Error::new(format!("cannot find variable {ident}"), ident_span));
                };
                self.build.store(var.ptr_to, rhs, var.tyl.layout, stmt_span);
            }
            Stmt::Expr(expr) => {
                self.lower_expr(expr, stmt_span)?;
            }
        }

        Ok(())
    }

    fn lower_expr(&mut self, expr: &ast::Expr, span: Span) -> Result<Operand> {
        match expr {
            ast::Expr::Atom(Atom::Char(c)) => Ok(Operand::Const(ConstValue::Int((*c).into()))),
            ast::Expr::Atom(Atom::Int(i)) => Ok(Operand::Const(ConstValue::Int(*i as _))),
            ast::Expr::Atom(Atom::Float(_)) => todo!("no floats"),
            ast::Expr::Atom(Atom::Ident(_)) => todo!("no idents"),
            ast::Expr::Atom(Atom::String(_)) => todo!("no string literals"),
            ast::Expr::Unary(_) => todo!("no unaries"),
            ast::Expr::Binary(binary) => {
                let lhs = self.lower_expr(&binary.lhs.0, binary.lhs.1)?;
                let rhs = self.lower_expr(&binary.rhs.0, binary.rhs.1)?;
                let kind = match binary.op {
                    ast::BinaryOp::Arith(ast::ArithOpKind::Mul) => BinKind::Mul,
                    ast::BinaryOp::Arith(ast::ArithOpKind::Div) => BinKind::Div,
                    ast::BinaryOp::Arith(ast::ArithOpKind::Mod) => BinKind::Mod,
                    ast::BinaryOp::Arith(ast::ArithOpKind::Add) => BinKind::Add,
                    ast::BinaryOp::Arith(ast::ArithOpKind::Sub) => BinKind::Sub,
                    ast::BinaryOp::Arith(ast::ArithOpKind::Shl) => todo!("shl"),
                    ast::BinaryOp::Arith(ast::ArithOpKind::Shr) => todo!("shr"),
                    ast::BinaryOp::Arith(ast::ArithOpKind::BitAnd) => todo!("&"),
                    ast::BinaryOp::Arith(ast::ArithOpKind::BitXor) => todo!("^"),
                    ast::BinaryOp::Arith(ast::ArithOpKind::BitOr) => todo!("|"),
                    ast::BinaryOp::LogicalAnd => todo!("no logical or"),
                    ast::BinaryOp::LogicalOr => todo!("no logical and"),
                    ast::BinaryOp::Comparison(ast::ComparisonKind::Lt) => BinKind::Lt,
                    ast::BinaryOp::Comparison(ast::ComparisonKind::Gt) => BinKind::Gt,
                    ast::BinaryOp::Comparison(ast::ComparisonKind::LtEq) => BinKind::Leq,
                    ast::BinaryOp::Comparison(ast::ComparisonKind::GtEq) => BinKind::Geq,
                    ast::BinaryOp::Comparison(ast::ComparisonKind::Eq) => BinKind::Eq,
                    ast::BinaryOp::Comparison(ast::ComparisonKind::Neq) => BinKind::Neq,
                    ast::BinaryOp::Comma => todo!("no comma"),
                    ast::BinaryOp::Index => todo!("no index"),
                    ast::BinaryOp::Assign(_) => todo!("no assign"),
                };

                let reg = self.build.binary(
                    kind,
                    lhs,
                    rhs,
                    span,
                    self.lcx.layout_of(self.lcx.intern_ty(TyKind::Void)),
                );

                Ok(Operand::Reg(reg))
            }
            Expr::Postfix(postfix) => {
                let lhs = self.lower_expr(&postfix.lhs.0, postfix.lhs.1)?;
                match &postfix.op {
                    ast::PostfixOp::Call(args) => {
                        let args = args
                            .iter()
                            .map(|(arg, sp)| self.lower_expr(arg, *sp))
                            .collect::<Result<_, _>>()?;

                        let reg = self.build.call(
                            self.lcx.layout_of(self.lcx.intern_ty(TyKind::Void)),
                            lhs,
                            args,
                            span,
                        );
                        Ok(Operand::Reg(reg))
                    }
                    ast::PostfixOp::Member(_) => todo!("member expr"),
                    ast::PostfixOp::ArrowMember(_) => todo!("arrow member expr"),
                    ast::PostfixOp::Increment => {
                        todo!("gotta have lvalues")
                    }
                    ast::PostfixOp::Decrement => todo!(),
                }
            }
        }
    }
}

#[derive(Debug)]
struct VariableInfo<'cx> {
    def_span: Span,
    tyl: TyLayout<'cx>,
    ptr_to: Register,
    decl_attr: DeclAttr,
}

fn lower_body<'cx>(
    // may be used later
    lcx: &LoweringCx<'cx>,
    body: &[(Stmt, Span)],
    def_span: Span,
    name: Symbol,
    ret_ty: Ty<'cx>,
) -> Result<Func<'cx>, Error> {
    let mut cx = FnLoweringCtxt {
        scopes: vec![FxHashMap::default()],
        build: FuncBuilder::new(name, def_span, ret_ty, lcx),
        lcx,
    };

    cx.lower_body(body)?;

    Ok(cx.build.finish())
}
