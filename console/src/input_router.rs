//! Terminal input routing.

/// The destination selected for one submitted line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InputRoute {
    /// An OID command without its leading colon.
    Oid(String),
    /// A command to execute through the configured shell.
    Shell(String),
}

/// Routes explicit OID commands and ordinary shell input.
#[derive(Clone, Debug, Default)]
pub struct InputRouter;

impl InputRouter {
    /// Classify input by the explicit OID `:` namespace.
    #[must_use]
    #[allow(clippy::unused_self)]
    pub fn route(&self, input: &str) -> InputRoute {
        let input = input.trim();
        input.strip_prefix(':').map_or_else(
            || InputRoute::Shell(input.to_owned()),
            |command| InputRoute::Oid(command.trim().to_owned()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{InputRoute, InputRouter};

    #[test]
    fn routes_shell_and_oid_namespaces() {
        let router = InputRouter;
        assert_eq!(
            router.route("ls -la"),
            InputRoute::Shell("ls -la".to_owned())
        );
        assert_eq!(router.route(":help"), InputRoute::Oid("help".to_owned()));
    }
}
