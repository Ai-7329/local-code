use super::loader::Skill;
use super::registry::SkillRegistry;

/// トリガー検出器
pub struct TriggerDetector<'a> {
    registry: &'a SkillRegistry,
}

impl<'a> TriggerDetector<'a> {
    pub fn new(registry: &'a SkillRegistry) -> Self {
        Self { registry }
    }

    /// 入力テキストからトリガーにマッチするスキルを検出
    pub fn detect(&self, input: &str) -> Vec<&Skill> {
        let mut matches = Vec::new();

        // /skill-name 形式のコマンド検出
        if let Some(stripped) = input.strip_prefix('/') {
            let skill_name = stripped.split_whitespace().next().unwrap_or("");
            if let Some(skill) = self.registry.get(skill_name) {
                matches.push(skill);
            }
        }

        // トリガーフレーズ検出
        for skill in self.registry.list() {
            if skill.matches_trigger(input)
                && !matches.iter().any(|s| s.metadata.name == skill.metadata.name)
            {
                matches.push(skill);
            }
        }

        // 自動実行スキルを優先
        matches.sort_by(|a, b| b.metadata.auto.cmp(&a.metadata.auto));

        matches
    }

    /// /skill-name 形式かチェック
    pub fn is_skill_command(input: &str) -> bool {
        input.starts_with('/') && !input.starts_with("/help")
            && !input.starts_with("/quit")
            && !input.starts_with("/plan")
            && !input.starts_with("/execute")
            && !input.starts_with("/clear")
    }

    /// コマンドからスキル名を抽出
    pub fn extract_skill_name(input: &str) -> Option<&str> {
        if Self::is_skill_command(input) {
            input[1..].split_whitespace().next()
        } else {
            None
        }
    }

    /// コマンドから引数を抽出
    pub fn extract_args(input: &str) -> Option<&str> {
        if Self::is_skill_command(input) {
            let parts: Vec<&str> = input[1..].splitn(2, char::is_whitespace).collect();
            if parts.len() > 1 {
                Some(parts[1].trim())
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_skill_command() {
        assert!(TriggerDetector::is_skill_command("/my-skill"));
        assert!(!TriggerDetector::is_skill_command("/help"));
        assert!(!TriggerDetector::is_skill_command("/plan"));
        assert!(!TriggerDetector::is_skill_command("regular message"));
    }

    #[test]
    fn test_extract_skill_name() {
        assert_eq!(TriggerDetector::extract_skill_name("/commit fix bug"), Some("commit"));
        assert_eq!(TriggerDetector::extract_skill_name("/review-pr 123"), Some("review-pr"));
        assert_eq!(TriggerDetector::extract_skill_name("not a command"), None);
    }

    #[test]
    fn test_extract_args() {
        assert_eq!(TriggerDetector::extract_args("/commit fix bug"), Some("fix bug"));
        assert_eq!(TriggerDetector::extract_args("/skill"), None);
    }
}
