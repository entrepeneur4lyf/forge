use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::anyhow;
use forge_env::Environment;
use forge_provider::ModelId;
use forge_server::{ChatRequest, ChatResponse, Server};
use forge_walker::Walker;
use regex::Regex;
use tokio_stream::StreamExt;

pub struct TestServer {
    pub server: Server,
    pub model: String,
    pub env: Environment,
    pub absolute_path: PathBuf,
    pub test_dir: PathBuf,
}

impl TestServer {
    pub async fn new<T: AsRef<Path>>(test_dir: T) -> anyhow::Result<Self> {
        let test_dir = test_dir.as_ref();
        let absolute_path = PathBuf::from(".").canonicalize()?.join(test_dir);

        let env = Environment::from_env(Some(absolute_path.clone()))
            .await
            .map_err(|e| anyhow!(e))?;
        let api_key = std::env::var("FORGE_KEY").map_err(|_| anyhow!("FORGE_KEY must be set"))?;
        let model = std::env::var("FORGE_MODEL").map_err(|_| anyhow!("FORGE_MODEL must be set"))?;
        Ok(Self {
            server: Server::new(env.clone(), api_key),
            model,
            env,
            absolute_path,
            test_dir: test_dir.into(),
        })
    }
    pub async fn chat<T: AsRef<str>>(&self, prompt: T) -> Vec<ChatResponse> {
        let file_paths = &self.env.files;
        let files = file_paths
            .iter()
            .map(|v| self.test_dir.join(v).to_str().unwrap().to_string())
            .collect::<Vec<_>>();

        let edited_prompt = format!("{} in the file(s) located at {} .Do not change the input file directly, create another file(s) and name it <file_name>_updated.ext",
                                    prompt.as_ref(),
                                    files.join(", "),
        );

        let req = ChatRequest::default()
            .message(edited_prompt)
            .model(ModelId::new(self.model.clone()));

        self.server
            .chat(req)
            .await
            .unwrap()
            .collect::<Vec<ChatResponse>>()
            .await
    }

    fn delete_file(file_path: &str) {
        let _ = std::fs::remove_file(file_path);
    }

    pub async fn create_patches(&self) -> anyhow::Result<String> {
        let updated_path_re = Regex::new(r"_updated\.(\w+)$").unwrap();
        let re = Regex::new(r"\.(\w+)$").unwrap();
        let mut files = match Walker::new(self.absolute_path.clone()).get().await {
            Ok(files) => files
                .into_iter()
                .filter(|f| !f.is_dir)
                .map(|f| {
                    self.absolute_path
                        .join(f.path)
                        .to_str()
                        .unwrap()
                        .to_string()
                })
                .collect(),
            Err(_) => vec![],
        };

        let mut map = HashMap::new();
        for file in files.iter() {
            let content =
                std::fs::read_to_string(file).map_err(|_| anyhow!("{} does not exist", file))?;
            map.insert(file.clone(), content);
        }

        files.retain(|f| !updated_path_re.is_match(f));

        let mut patches = vec![];
        for file in files {
            let updated_file = re.replace(&file, "_updated.$1").to_string();

            let patch = diffy::create_patch(
                map.get(&file)
                    .ok_or(anyhow!("File not found (this should never happen)"))?,
                map.get(&updated_file)
                    .ok_or(anyhow!("Updated file {} not found", updated_file))?,
            );

            let mut patch_output = vec![];
            diffy::PatchFormatter::new()
                .write_patch_into(&patch, &mut patch_output)
                .expect("Failed to format patch");

            let patch_output = String::from_utf8(patch_output)?;

            patches.push((
                PathBuf::from(file)
                    .strip_prefix(&self.absolute_path)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string(),
                patch_output,
            ));
            Self::delete_file(&updated_file);
        }

        let patches = patches
            .into_iter()
            .map(|(file, patch)| format!("{}:\n{}", file, patch))
            .collect::<Vec<_>>()
            .join("----------------\n");

        Ok(patches)
    }
}
