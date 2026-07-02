use crate::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiagnosticCode(String);

impl DiagnosticCode {
    #[must_use]
    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for DiagnosticCode {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for DiagnosticCode {
    fn from(s: String) -> Self {
        Self(s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: DiagnosticCode,
    pub message: String,
    pub span: Span,
}

impl Diagnostic {
    #[must_use]
    pub fn error(code: impl Into<DiagnosticCode>, message: impl Into<String>, span: Span) -> Self {
        Self {
            severity: Severity::Error,
            code: code.into(),
            message: message.into(),
            span,
        }
    }

    #[must_use]
    pub fn warning(
        code: impl Into<DiagnosticCode>,
        message: impl Into<String>,
        span: Span,
    ) -> Self {
        Self {
            severity: Severity::Warning,
            code: code.into(),
            message: message.into(),
            span,
        }
    }

    #[must_use]
    pub fn note(code: impl Into<DiagnosticCode>, message: impl Into<String>, span: Span) -> Self {
        Self {
            severity: Severity::Note,
            code: code.into(),
            message: message.into(),
            span,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct DiagnosticBag {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticBag {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn push(&mut self, diag: Diagnostic) {
        self.diagnostics.push(diag);
    }

    pub fn extend<I: IntoIterator<Item = Diagnostic>>(&mut self, iter: I) {
        self.diagnostics.extend(iter);
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Diagnostic> {
        self.diagnostics.iter()
    }

    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
    }

    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }
}

impl<'a> IntoIterator for &'a DiagnosticBag {
    type Item = &'a Diagnostic;
    type IntoIter = std::slice::Iter<'a, Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.diagnostics.iter()
    }
}

impl Extend<Diagnostic> for DiagnosticBag {
    fn extend<I: IntoIterator<Item = Diagnostic>>(&mut self, iter: I) {
        self.diagnostics.extend(iter);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_bag_has_no_errors() {
        let bag = DiagnosticBag::new();
        assert!(!bag.has_errors());
    }

    #[test]
    fn push_adds_diagnostic() {
        let mut bag = DiagnosticBag::new();
        bag.push(Diagnostic::error("E0001", "boom", Span::new(0, 4)));
        assert!(bag.has_errors());
        assert_eq!(bag.len(), 1);
    }

    #[test]
    fn extend_appends_diagnostics() {
        let mut a = DiagnosticBag::new();
        a.push(Diagnostic::error("E0001", "from a", Span::new(0, 1)));

        let mut b = DiagnosticBag::new();
        b.push(Diagnostic::warning("W0001", "from b", Span::new(2, 3)));

        a.extend(b.diagnostics);
        assert_eq!(a.len(), 2);
        assert!(a.has_errors());
    }

    #[test]
    fn extend_accepts_filtered_iterator() {
        let mut bag = DiagnosticBag::new();
        bag.push(Diagnostic::error("E0001", "kept", Span::new(0, 1)));

        let incoming = vec![
            Diagnostic::error("E0002", "a", Span::new(2, 3)),
            Diagnostic::warning("W0001", "dropped", Span::new(4, 5)),
            Diagnostic::error("E0003", "b", Span::new(6, 7)),
        ];
        bag.extend(
            incoming
                .into_iter()
                .filter(|d| d.severity == Severity::Error),
        );

        assert_eq!(bag.len(), 3);
        assert_eq!(bag.errors().count(), 3);
    }

    #[test]
    fn errors_filter_returns_only_errors() {
        let mut bag = DiagnosticBag::new();
        bag.push(Diagnostic::error("E0001", "err", Span::new(0, 1)));
        bag.push(Diagnostic::warning("W0001", "warn", Span::new(2, 3)));
        bag.push(Diagnostic::note("N0001", "note", Span::new(4, 5)));

        let errors: Vec<&Diagnostic> = bag.errors().collect();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].severity, Severity::Error);
    }

    #[test]
    fn into_iterator_iterates_all_diagnostics() {
        let mut bag = DiagnosticBag::new();
        bag.push(Diagnostic::error("E0001", "first", Span::new(0, 1)));
        bag.push(Diagnostic::error("E0002", "second", Span::new(2, 3)));

        let collected: Vec<&Diagnostic> = (&bag).into_iter().collect();
        assert_eq!(collected.len(), 2);
        assert_eq!(collected[0].message, "first");
        assert_eq!(collected[1].message, "second");
    }

    #[test]
    fn severity_variants_are_distinct() {
        assert_ne!(Severity::Error, Severity::Warning);
        assert_ne!(Severity::Warning, Severity::Note);
        assert_ne!(Severity::Error, Severity::Note);
    }

    #[test]
    fn diagnostic_code_constructors_round_trip() {
        let from_str = DiagnosticCode::from("E0001");
        let from_string = DiagnosticCode::from(String::from("E0002"));
        let from_new = DiagnosticCode::new("E0003");
        assert_eq!(from_str.as_str(), "E0001");
        assert_eq!(from_string.as_str(), "E0002");
        assert_eq!(from_new.as_str(), "E0003");
    }
}
