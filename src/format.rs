use std::fmt::Display;

use crate::engine::Engine;


impl Display for Engine {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		for (idx, node) in self.nodes() {
			write!(f, "{idx} {} {}\n", node.id, node.node)?;
			
			for input in &node.inputs {
				if let Some(input) = input {
					writeln!(f, "in {}.{}", input.node, input.output)?;
				} else {
					writeln!(f, "in")?;
				}
			}

			writeln!(f)?;
		}

		Ok(())
	}
}