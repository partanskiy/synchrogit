pub mod load;
pub mod model;

pub use load::{
    LoadedConfig, config_candidates, load, load_from_candidates, load_from_path, parse_str,
};
pub use model::{
    Config, ConflictPolicy, DEFAULT_DEBOUNCE, DEFAULT_INTERVAL, DefaultsConfig, RepoConfig,
    ResolvedRepoConfig,
};
