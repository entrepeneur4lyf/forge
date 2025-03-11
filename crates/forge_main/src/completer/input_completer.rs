use std::path::PathBuf;

use forge_walker::Walker;
use reedline::{Completer, Suggestion};
use tracing::trace;

use crate::completer::search_term::SearchTerm;
use crate::completer::CommandCompleter;

#[derive(Clone)]
pub struct InputCompleter {
    walker: Walker,
}

impl InputCompleter {
    pub fn new(cwd: PathBuf) -> Self {
        let walker = Walker::max_all().cwd(cwd).skip_binary(true);
        Self { walker }
    }
}

impl Completer for InputCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        if line.starts_with("/") {
            // if the line starts with '/' it's probably a command, so we delegate to the
            // command completer.
            let result = CommandCompleter.complete(line, pos);
            if !result.is_empty() {
                return result;
            }
        }

        if let Some(query) = SearchTerm::new(line, pos).process() {
            let files = self.walker.get_blocking().unwrap_or_default();
            let suggestions = files
                .into_iter()
                .filter(|file| !file.is_dir())
                .filter_map(|file| {
                    if let Some(file_name) = file.file_name.as_ref() {
                        let file_name_lower = file_name.to_lowercase();
                        let query_lower = query.term.to_lowercase();
                        if file_name_lower.contains(&query_lower) {
                            let replacement_value = file_name.to_string();
                            
                            let description = if file.path.len() > file_name.len() {
                                Some(format!("File: {}", file.path))
                            } else {
                                Some("File".to_string())
                            };
                            
                            Some(Suggestion {
                                value: replacement_value,
                                description,
                                style: None,
                                extra: None,
                                span: query.span,
                                append_whitespace: true,
                            })
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
                
            // Return the suggestions without selecting any (don't auto-select on tab)
            // When the user selects one, it will replace just the text after '@'
            suggestions
        } else {
            vec![]
        }
    }
}
