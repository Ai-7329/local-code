pub mod loader;
pub mod registry;
pub mod trigger;
pub mod executor;
pub mod superpowers;
pub mod embedded;

pub use loader::{Skill, SkillMetadata};
pub use registry::SkillRegistry;
pub use trigger::TriggerDetector;
pub use executor::{SkillExecutor, SkillContext, SkillResult};
pub use superpowers::{SuperpowersCommand, load_superpowers_commands};
pub use embedded::EmbeddedSuperpowers;
