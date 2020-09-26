use crate::error::RuntimeError;
use crate::parser::{Assignment, Binary, Expr, Grouping, Stmt, Ternary, Unary, Var_Decl};
use crate::token::{self, Literal::*, TokenType::*};
use std::collections::{
    HashMap, hash_map::RandomState, hash_map::RawEntryMut, hash_map::RawOccupiedEntryMut,
};

type Result_Interpreter = Result<token::Literal, RuntimeError>;

pub struct Interpreter {
    env: Environment,
}

struct Environment {
    env: HashMap<String, token::Literal>,
    parent_env: Option<Box<Environment>>,
}

impl Environment {
    fn new() -> Environment {
        Environment {
            env: HashMap::new(),
            parent_env: None,
        }
    }

    fn set_parent(&mut self, parent_env: Environment) {
        self.parent_env = Some(Box::new(parent_env));
    }

    fn disown_parent(&mut self) -> Option<Box<Environment>> {
        self.parent_env.take()
    }

    fn define(&mut self, ident: String, literal: Option<token::Literal>) {
        if let Some(value) = literal {
            self.env.insert(ident, value);
        } else {
            self.env.insert(ident, LoxNil);
        }
    }

    fn get_entry(
        &mut self,
        ident: String,
    ) -> Result<RawOccupiedEntryMut<String, token::Literal, RandomState>, RuntimeError> {
        if let RawEntryMut::Occupied(entry) = self.env.raw_entry_mut().from_key(&ident) {
            return Ok(entry);
        } else {
            if let None = self.parent_env {
                return Err(RuntimeError::UndefinedVariable(ident));
            } else {
                let parent_env = self.parent_env.as_mut().unwrap();
                return Ok(parent_env.get_entry(ident)?);
            }
        };
    }

    fn get(&mut self, ident: String) -> Result_Interpreter {
        let entry = self.get_entry(ident)?;
        Ok(entry.get().clone())
    }

    fn assign(&mut self, ident: String, literal: token::Literal) -> Result_Interpreter {
        let mut entry = self.get_entry(ident)?;
        let value = entry.get_mut();
        *value = literal.clone();
        Ok(literal)
    }
}

impl Interpreter {
    pub fn new() -> Interpreter {
        Interpreter {
            env: Environment::new(),
        }
    }

    pub fn interpret(&mut self, stmts: Vec<Stmt>) -> Result<(), RuntimeError> {
        for stmt in stmts.into_iter() {
            self.execute(stmt)?
        }
        Ok(())
    }

    pub fn execute(&mut self, stmt: Stmt) -> Result<(), RuntimeError> {
        match stmt {
            Stmt::Expr(expr) => {
                self.evaluate(expr)?;
            }
            Stmt::Print(expr) => {
                let res = self.evaluate(expr)?;
                println!("{}", res);
            }
            Stmt::Var_Decl(Var_Decl {
                ident: token::Token { lexeme, .. },
                initializer: Some(expr),
            }) => {
                let res = self.evaluate(expr)?;
                self.env.define(lexeme, Some(res));
            }
            Stmt::Var_Decl(Var_Decl {
                ident: token::Token { lexeme, .. },
                initializer: None,
            }) => {
                self.env.define(lexeme, None);
            }
            _ => panic!(),
        }
        Ok(())
    }

    fn evaluate(&mut self, expr: Expr) -> Result_Interpreter {
        match expr {
            Expr::Literal(literal) => Ok(literal),
            Expr::Grouping(grouping) => self.eval_grouping(grouping),
            Expr::Unary(unary) => self.eval_unary(unary),
            Expr::Binary(binary) => self.eval_binary(binary),
            Expr::Ternary(ternary) => self.eval_ternary(ternary),
            Expr::Variable(token) => self.env.get(token.lexeme),
            Expr::Assignment(assignment) => self.eval_assignment(assignment),
            _ => panic!(),
        }
    }

    fn eval_assignment(&mut self, assignment_expr: Assignment) -> Result_Interpreter {
        let Assignment { ident, expression } = assignment_expr;
        let r_value = self.evaluate(*expression)?;
        self.env.assign(ident.lexeme, r_value)
    }

    fn eval_grouping(&mut self, group_expr: Grouping) -> Result_Interpreter {
        self.evaluate(*group_expr.expression)
    }

    fn eval_unary(&mut self, unary_expr: Unary) -> Result_Interpreter {
        let Unary { operator, right } = unary_expr;
        let right_val = self.evaluate(*right)?;

        let ret = match (&operator.token_type, right_val) {
            (MINUS, LoxNumber(actual_val)) => LoxNumber(-actual_val),
            (BANG, LoxValue) => LoxBool(!self.isTruthy(LoxValue)),
            (_, LoxType) => Err(RuntimeError::UnaryTypeError(operator, LoxType))?,
        };
        Ok(ret)
    }

    fn eval_binary(&mut self, bin_expr: Binary) -> Result_Interpreter {
        let Binary {
            left,
            operator,
            right,
        } = bin_expr;
        let left_val = self.evaluate(*left)?;
        let right_val = self.evaluate(*right)?;
        let ret = match (left_val, right_val) {
            (LoxNumber(left_num), LoxNumber(right_num)) => match &operator.token_type {
                PLUS => LoxNumber(left_num + right_num),
                MINUS => LoxNumber(left_num - right_num),
                STAR => LoxNumber(left_num * right_num),
                SLASH => LoxNumber(left_num / right_num),
                GREATER => LoxBool(left_num > right_num),
                GREATER_EQUAL => LoxBool(left_num >= right_num),
                LESS => LoxBool(left_num < right_num),
                LESS_EQUAL => LoxBool(left_num <= right_num),
                EQUAL_EQUAL => LoxBool(left_num == right_num),
                BANG_EQUAL => LoxBool(left_num != right_num),
                _ => Err(RuntimeError::BinaryTypeError(
                    LoxNumber(left_num),
                    operator,
                    LoxNumber(right_num),
                ))?,
            },
            (LoxString(left_string), LoxString(right_string)) => match &operator.token_type {
                PLUS => LoxString(format!("{}{}", left_string, right_string)),
                _ => Err(RuntimeError::BinaryTypeError(
                    LoxString(left_string),
                    operator,
                    LoxString(right_string),
                ))?,
            },
            (AnyTypeLeft, AnyTypeRight) => match &operator.token_type {
                EQUAL_EQUAL => LoxBool(AnyTypeLeft == AnyTypeRight),
                BANG_EQUAL => LoxBool(AnyTypeLeft != AnyTypeRight),
                _ => Err(RuntimeError::BinaryTypeError(
                    AnyTypeLeft,
                    operator,
                    AnyTypeRight,
                ))?,
            },
            _ => panic!(),
        };
        Ok(ret)
    }

    fn eval_ternary(&mut self, tern_expr: Ternary) -> Result_Interpreter {
        let Ternary {
            condition,
            if_true,
            if_false,
            ..
        } = tern_expr;
        let condition = self.evaluate(*condition)?;
        let condition = self.isTruthy(condition);
        let if_true = self.evaluate(*if_true)?;
        if condition {
            Ok(if_true)
        } else {
            let if_false = self.evaluate(*if_false)?;
            Ok(if_false)
        }
    }

    fn isTruthy(&self, literal: token::Literal) -> bool {
        match literal {
            LoxBool(val) => val,
            LoxNil => false,
            _ => true,
        }
    }
}
