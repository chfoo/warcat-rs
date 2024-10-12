use regex::Regex;

use crate::header::WarcHeader;

#[derive(Debug, Clone)]
pub struct FieldFilter {
    includes: Vec<(String, Option<String>)>,
    excludes: Vec<(String, Option<String>)>,
    include_patterns: Vec<(String, Regex)>,
    exclude_patterns: Vec<(String, Regex)>,
}

impl FieldFilter {
    pub fn new() -> Self {
        Self {
            includes: Vec::new(),
            excludes: Vec::new(),
            include_patterns: Vec::new(),
            exclude_patterns: Vec::new(),
        }
    }

    pub fn add_include(&mut self, rule: &str) {
        if let Some((name, value)) = rule.split_once(":") {
            self.includes
                .push((name.to_string(), Some(value.to_string())));
        } else {
            self.includes.push((rule.to_string(), None));
        }
    }

    pub fn add_exclude(&mut self, rule: &str) {
        if let Some((name, value)) = rule.split_once(":") {
            self.excludes
                .push((name.to_string(), Some(value.to_string())));
        } else {
            self.excludes.push((rule.to_string(), None));
        }
    }

    pub fn add_include_pattern(&mut self, rule: &str) -> anyhow::Result<()> {
        let (name, value) = rule.split_once(":").unwrap_or((rule, ""));

        self.include_patterns
            .push((name.to_string(), Regex::new(value)?));

        Ok(())
    }

    pub fn add_exclude_pattern(&mut self, rule: &str) -> anyhow::Result<()> {
        let (name, value) = rule.split_once(":").unwrap_or((rule, ""));

        self.exclude_patterns
            .push((name.to_string(), Regex::new(value)?));

        Ok(())
    }

    pub fn is_allow(&self, header: &WarcHeader) -> bool {
        for (rule_name, rule_value) in &self.excludes {
            if let Some(rule_value) = rule_value {
                for value in header.fields.get_all(rule_name) {
                    if value == rule_value {
                        return false;
                    }
                }
            } else if header.fields.contains_name(rule_name) {
                return false;
            }
        }

        for (rule_name, value_pattern) in &self.exclude_patterns {
            for value in header.fields.get_all(rule_name) {
                if value_pattern.is_match(value) {
                    return false;
                }
            }
        }

        for (rule_name, rule_value) in &self.includes {
            if let Some(rule_value) = rule_value {
                for value in header.fields.get_all(rule_name) {
                    if value == rule_value {
                        return true;
                    }
                }
            } else if header.fields.contains_name(rule_name) {
                return true;
            }
        }

        for (rule_name, value_pattern) in &self.include_patterns {
            for value in header.fields.get_all(rule_name) {
                if value_pattern.is_match(value) {
                    return true;
                }
            }
        }

        self.includes.is_empty() && self.include_patterns.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter() {
        let mut header1 = WarcHeader::empty();
        header1.fields.insert("n".to_string(), "cat".to_string());
        let mut header2 = WarcHeader::empty();
        header2.fields.insert("n".to_string(), "dog".to_string());
        let mut header3 = WarcHeader::empty();
        header3.fields.insert("n".to_string(), "bird".to_string());
        let mut header4 = WarcHeader::empty();
        header4
            .fields
            .insert("n".to_string(), "cat-and-dog".to_string());

        let mut filter = FieldFilter::new();
        filter.add_include("n:dog");
        filter.add_exclude("n:cat");

        assert!(!filter.is_allow(&header1));
        assert!(filter.is_allow(&header2));
        assert!(!filter.is_allow(&header3));
        assert!(!filter.is_allow(&header4));
    }

    #[test]
    fn test_filter_empty_value() {
        let mut header1 = WarcHeader::empty();
        header1.fields.insert("a".to_string(), "".to_string());
        let mut header2 = WarcHeader::empty();
        header2.fields.insert("b".to_string(), "".to_string());

        let mut filter = FieldFilter::new();
        filter.add_include("a");
        filter.add_exclude("b");

        assert!(filter.is_allow(&header1));
        assert!(!filter.is_allow(&header2));
    }

    #[test]
    fn test_filter_regex() {
        let mut header1 = WarcHeader::empty();
        header1.fields.insert("n".to_string(), "cat".to_string());
        let mut header2 = WarcHeader::empty();
        header2.fields.insert("n".to_string(), "dog".to_string());
        let mut header3 = WarcHeader::empty();
        header3.fields.insert("n".to_string(), "bird".to_string());
        let mut header4 = WarcHeader::empty();
        header4
            .fields
            .insert("n".to_string(), "cat-and-dog".to_string());

        let mut filter = FieldFilter::new();
        filter.add_include_pattern(r"n:\bdog\b").unwrap();
        filter.add_exclude_pattern(r"n:\bcat\b").unwrap();

        assert!(!filter.is_allow(&header1));
        assert!(filter.is_allow(&header2));
        assert!(!filter.is_allow(&header3));
        assert!(!filter.is_allow(&header4));
    }
}
