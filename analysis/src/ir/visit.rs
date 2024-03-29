use super::{BasicBlock, ConstValue, Func, Operand, Register, Statement, StatementKind};

pub trait Visitor {
    fn visit_func(&mut self, func: &Func<'_>) {
        self.super_func(func);
    }
    fn visit_bb(&mut self, bb: &BasicBlock) {
        self.super_bb(bb);
    }
    fn visit_statement(&mut self, stmt: &Statement) {
        self.super_statement(stmt);
    }
    fn visit_operand(&mut self, op: Operand) {
        self.super_operand(op);
    }
    fn visit_reg(&mut self, _: Register) {}
    fn visit_const(&mut self, _: ConstValue) {}

    fn super_func(&mut self, func: &Func<'_>) {
        for bb in &func.bbs {
            self.visit_bb(bb);
        }
    }

    fn super_bb(&mut self, bb: &BasicBlock) {
        for stmt in &bb.statements {
            self.visit_statement(stmt);
        }
    }

    fn super_statement(&mut self, stmt: &Statement) {
        match stmt.kind {
            StatementKind::Alloca {
                result,
                size: _,
                align: _,
            } => {
                self.visit_reg(result);
            }
            StatementKind::Store {
                ptr,
                value,
                size: _,
                align: _,
            } => {
                self.visit_operand(ptr);
                self.visit_operand(value);
            }
            StatementKind::Load {
                result,
                ptr,
                size: _,
                align: _,
            } => {
                self.visit_reg(result);
                self.visit_operand(ptr);
            }
            StatementKind::BinOp {
                kind: _,
                lhs,
                rhs,
                result,
            } => {
                self.visit_reg(result);
                self.visit_operand(lhs);
                self.visit_operand(rhs);
            }
            StatementKind::UnaryOperation {
                rhs,
                kind: _,
                result,
            } => {
                self.visit_reg(result);
                self.visit_operand(rhs);
            }
            StatementKind::PtrOffset {
                result,
                ptr,
                amount,
            } => {
                self.visit_reg(result);
                self.visit_operand(ptr);
                self.visit_operand(amount);
            }
            StatementKind::Call {
                result,
                func,
                ref args,
            } => {
                self.visit_reg(result);
                self.visit_operand(func);
                for &arg in args {
                    self.visit_operand(arg);
                }
            }
        }
    }

    fn super_operand(&mut self, op: Operand) {
        match op {
            Operand::Reg(reg) => self.visit_reg(reg),
            Operand::Const(c) => self.visit_const(c),
        }
    }
}
