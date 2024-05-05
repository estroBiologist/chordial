use std::fmt::Display;

use crate::engine::Engine;


impl Display for Engine {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		for (idx, node) in self.nodes() {
			write!(f, "{idx} {}\n", node.id)?;
			
			for input in &node.inputs {
				write!(f, "in")?;

				for input_node in &input.0 {
					write!(f, " {}.{}", input_node.node, input_node.output)?;
				}

				write!(f, "\n")?;
			}

			for (_, value) in node.get_params() {
				writeln!(f, "param {value}")?;
			}

			writeln!(f)?;
		}

		Ok(())
	}
}