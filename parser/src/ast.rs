use std::fmt::Debug;

use bitflags::bitflags;
use dbg_pls::DebugPls;

use crate::sym::Symbol;
pub use crate::Spanned;

pub type Ident = Spanned<Symbol>;

//
// --- Expr
//

#[derive(Debug, DebugPls)]
pub enum Atom {
    Ident(Ident),
    Int(u128),
    Float(f64),
    String(Vec<u8>),
    Char(u8),
}

#[derive(Debug, DebugPls)]
pub enum UnaryOp {
    Increment,
    Decrement,
    AddrOf,
    Deref,
    Plus,
    Minus,
    Tilde,
    Bang,
}

#[derive(Debug, DebugPls)]
pub enum ArithOpKind {
    Mul,
    Div,
    Mod,
    Add,
    Sub,
    Shl,
    Shr,
    BitAnd,
    BitXor,
    BitOr,
}

#[derive(Debug, DebugPls)]
pub enum ComparisonKind {
    Lt,
    Gt,
    LtEq,
    GtEq,
    Eq,
    Neq,
}

#[derive(Debug, DebugPls)]
pub enum BinaryOp {
    Arith(ArithOpKind),
    LogicalAnd,
    LogicalOr,
    Comparison(ComparisonKind),
    Comma,
    Index, // lhs[rhs]
    Assign(Option<ArithOpKind>),
}

#[derive(Debug, DebugPls)]
pub struct ExprUnary {
    pub rhs: Box<Spanned<Expr>>,
    pub op: UnaryOp,
}

#[derive(Debug, DebugPls)]
pub struct ExprBinary {
    pub lhs: Box<Spanned<Expr>>,
    pub rhs: Box<Spanned<Expr>>,
    pub op: BinaryOp,
}

#[derive(Debug, DebugPls)]
pub enum PostfixOp {
    Call(Vec<Spanned<Expr>>),
    Member(Ident),
    ArrowMember(Ident),
    Increment,
    Decrement,
}

#[derive(Debug, DebugPls)]
pub struct ExprPostfix {
    pub lhs: Box<Spanned<Expr>>,
    pub op: PostfixOp,
}

#[derive(Debug, DebugPls)]
pub enum Expr {
    Atom(Atom),
    Unary(ExprUnary),
    Binary(ExprBinary),
    Postfix(ExprPostfix),
}

//
// --- Statements
//

#[derive(Debug, DebugPls)]
pub enum Stmt {
    Decl(Decl),
    Labeled {
        label: Ident,
        stmt: Box<Spanned<Stmt>>,
    },
    Compound(Vec<Spanned<Stmt>>),
    If {
        cond: Spanned<Expr>,
        then: Vec<Spanned<Stmt>>,
        otherwise: Option<Vec<Spanned<Stmt>>>,
    },
    Switch,
    While {
        cond: Expr,
        body: Vec<Spanned<Stmt>>,
    },
    For {
        init_decl: Option<Spanned<Decl>>,
        init_expr: Option<Spanned<Expr>>,
        cond: Option<Spanned<Expr>>,
        post: Option<Spanned<Expr>>,
        body: Vec<Spanned<Stmt>>,
    },
    Goto(Ident),
    Continue,
    Break,
    Return(Option<Spanned<Expr>>),
    Expr(Expr),
}

//
// --- Types and decls and garbage whatever
//

#[derive(Debug, DebugPls, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntSign {
    Signed,
    Unsigned,
}

impl Default for IntSign {
    fn default() -> Self {
        // C defaults to unsigned for integers.
        Self::Signed
    }
}

#[derive(Debug, DebugPls, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
// N.B: Ord, order matters.
pub enum IntTyKind {
    Bool,
    Char,
    Short,
    Int,
    Long,
    LongLong,
}

#[derive(Debug, DebugPls, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IntTy(pub IntSign, pub IntTyKind);

#[derive(Debug, DebugPls, Clone)]
pub enum TypeSpecifier {
    Void,
    Char,
    Integer(IntTy),
    Float,
    Double,
    LongDouble,
    // TODO
    // complex
    // atomic-type-specifier
    // struct-or-union-specifier
    // enum-specifier
    // typedef-name
}

bitflags! {
    pub struct DeclAttr: u8 {
        const EXTERN = 0b00000001;
        const STATIC = 0b00000010;
        const THREAD_LOCAL = 0b00000100;
    }
}

impl DebugPls for DeclAttr {
    fn fmt(&self, f: dbg_pls::Formatter<'_>) {
        use std::fmt::Write;
        let mut string = String::new();
        write!(string, "{:?}", self).unwrap();
        DebugPls::fmt(&string, f);
    }
}

#[derive(Debug, DebugPls, Clone)]
pub struct DeclSpec {
    pub ty: TypeSpecifier,
    pub attrs: DeclAttr,
}

#[derive(Debug, DebugPls)]
pub enum Decl {
    Normal(NormalDecl),
    StaticAssert,
}

#[derive(Debug, DebugPls)]
pub struct InitDecl {
    pub declarator: Declarator,
    pub init: Option<Spanned<Expr>>,
}

#[derive(Debug, DebugPls)]
pub struct NormalDecl {
    pub decl_spec: DeclSpec,
    pub init_declarators: Vec<Spanned<InitDecl>>,
}

#[derive(Debug, DebugPls, Clone)]
pub struct FunctionParamDecl {
    pub decl_spec: Spanned<DeclSpec>,
    pub declarator: Spanned<Declarator>,
}

#[derive(Debug, DebugPls, Clone)]
pub enum DirectDeclarator {
    Ident(Ident),
    WithParams {
        ident: Ident,
        params: Vec<FunctionParamDecl>,
    },
}

#[derive(Debug, DebugPls, Clone)]
pub struct Declarator {
    pub decl: DirectDeclarator,
    pub pointer: bool,
}

#[derive(Debug, DebugPls)]
pub struct FunctionDef {
    pub decl: Decl,
    pub body: Vec<Spanned<Stmt>>,
}

#[derive(Debug, DebugPls)]
pub enum ExternalDecl {
    Decl(Decl),
    FunctionDef(FunctionDef),
}

pub type TranslationUnit = Vec<Spanned<ExternalDecl>>;

impl Decl {
    pub fn unwrap_normal(&self) -> &NormalDecl {
        match self {
            Decl::Normal(decl) => decl,
            Decl::StaticAssert => {
                panic!("Expected normal declaration, found static assert declaration")
            }
        }
    }
}

impl DirectDeclarator {
    pub fn unwrap_with_params(&self) -> (&Ident, &Vec<FunctionParamDecl>) {
        match self {
            DirectDeclarator::Ident(_) => {
                panic!("Expected declarator with parameters, found single identifier declarator1")
            }
            DirectDeclarator::WithParams { ident, params } => (ident, params),
        }
    }

    pub fn name(&self) -> Ident {
        match *self {
            DirectDeclarator::Ident(ident) => ident,
            DirectDeclarator::WithParams { ident, .. } => ident,
        }
    }
}

impl IntSign {
    pub fn signed(self) -> bool {
        matches!(self, Self::Signed)
    }

    pub fn unsigned(self) -> bool {
        matches!(self, Self::Unsigned)
    }
}
