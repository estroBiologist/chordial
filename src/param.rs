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
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum ParamKind {
	String,
	Float,
	Int,
}

impl Display for ParamValue {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			ParamValue::String(string) => write!(f, "s:{string}"),
			ParamValue::Float(float) => write!(f, "f:{float}"),
			ParamValue::Int(int) => write!(f, "i:{int}"),
		}
	}
}

impl ParamValue {
	pub fn parse(string: &str) -> Self {
		match string.chars().next().unwrap() {
			's' => ParamValue::String(string[2..].to_string()),
			'f' => ParamValue::Float(string[2..].parse().unwrap()),
			'i' => ParamValue::Int(string[2..].parse().unwrap()),
			other => panic!("invalid parameter prefix: `{other}`"),
		}
	}

	pub fn kind(&self) -> ParamKind {
		match self {
			ParamValue::String(_) => ParamKind::String,
			ParamValue::Float(_) => ParamKind::Float,
			ParamValue::Int(_) => ParamKind::Int,
		}
	}

	pub fn from_desc(param: Parameter) -> Self {
		match param.kind {
			ParamKind::String => ParamValue::String(String::new()),
			ParamKind::Float => ParamValue::Float(0.0),
			ParamKind::Int => ParamValue::Int(0),
		}
	}

	pub fn set_string(&mut self, value: String) {
		let ParamValue::String(string) = self else {
			panic!("can't assign String value to {self}");
		};
		*string = value;
	}

	pub fn set_int(&mut self, value: i64) {
		let ParamValue::Int(int) = self else {
			panic!("can't assign Int value to {self}");
		};
		*int = value;
	}

	pub fn set_float(&mut self, value: f64) {
		let ParamValue::Float(float) = self else {
			panic!("can't assign Float value to {self}");
		};
		*float = value;
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
			(this, param) => panic!("mismatched ParamKind assignment ({this}, {param})")
		}
	}
}