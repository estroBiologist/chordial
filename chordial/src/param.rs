use std::fmt::Display;


#[derive(Debug, Copy, Clone)]
pub struct Parameter {
	pub kind: ParamKind,
	pub text: &'static str,
}

#[derive(Debug, Clone)]
pub enum ParamValue {
	String(String),
	Float(f64),
	Int(i64),
	Bool(bool),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum ParamKind {
	String,
	Float,
	Int,
	Bool,
}

impl Display for ParamValue {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			ParamValue::String(string) => write!(f, "s:{string}"),
			ParamValue::Float(float) => write!(f, "f:{float}"),
			ParamValue::Int(int) => write!(f, "i:{int}"),
			ParamValue::Bool(boolean) => write!(f, "b:{boolean}"),
		}
	}
}

impl ParamValue {
	pub fn parse(string: &str) -> Self {
		match string.chars().next().unwrap() {
			's' => ParamValue::String(string[2..].to_string()),
			'f' => ParamValue::Float(string[2..].parse().unwrap()),
			'i' => ParamValue::Int(string[2..].parse().unwrap()),
			'b' => ParamValue::Bool(string[2..].parse().unwrap()),
			other => panic!("invalid parameter prefix: `{other}`"),
		}
	}

	pub fn kind(&self) -> ParamKind {
		match self {
			ParamValue::String(_) => ParamKind::String,
			ParamValue::Float(_) => ParamKind::Float,
			ParamValue::Int(_) => ParamKind::Int,
			ParamValue::Bool(_) => ParamKind::Bool,
		}
	}

	pub fn from_desc(param: Parameter) -> Self {
		match param.kind {
			ParamKind::String => ParamValue::String(String::new()),
			ParamKind::Float => ParamValue::Float(0.0),
			ParamKind::Int => ParamValue::Int(0),
			ParamKind::Bool => ParamValue::Bool(false),
		}
	}

	pub fn set_string(&mut self, value: String) {
		let ParamValue::String(string) = self else {
			panic!("can't assign String value to {self}")
		};

		*string = value;
	}

	pub fn set_int(&mut self, value: i64) {
		let ParamValue::Int(int) = self else {
			panic!("can't assign Int value to {self}")
		};

		*int = value;
	}

	pub fn set_float(&mut self, value: f64) {
		let ParamValue::Float(float) = self else {
			panic!("can't assign Float value to {self}")
		};

		*float = value;
	}

	pub fn set_bool(&mut self, value: bool) {
		let ParamValue::Bool(boolean) = self else {
			panic!("can't assign Bool value to {self}")
		};

		*boolean = value;
	}

	pub fn set(&mut self, param: ParamValue) {
		match (self, param) {
			(ParamValue::String(a), ParamValue::String(b)) => {
				*a = b
			}
			
			(ParamValue::Float(a), ParamValue::Float(b)) => {
				*a = b
			}
			
			(ParamValue::Int(a), ParamValue::Int(b)) => {
				*a = b
			}

			(ParamValue::Bool(a), ParamValue::Bool(b)) => {
				*a = b
			}

			(this, param) => panic!("mismatched ParamKind assignment ({this}, {param})")
		}
	}
}

impl From<String> for ParamValue {
	fn from(value: String) -> Self {
		ParamValue::String(value)
	}
}

impl From<i64> for ParamValue {
	fn from(value: i64) -> Self {
		ParamValue::Int(value)
	}
}

impl From<f64> for ParamValue {
	fn from(value: f64) -> Self {
		ParamValue::Float(value)
	}
}

impl From<bool> for ParamValue {
	fn from(value: bool) -> Self {
		ParamValue::Bool(value)
	}
}