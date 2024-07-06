#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct CompletionItem {
    pub label: String,
    pub filter_text: Option<String>,
    pub insert_text: Option<String>,
}
