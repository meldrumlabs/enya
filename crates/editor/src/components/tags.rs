use std::collections::{HashMap, HashSet};

/// A hierarchical tag path (e.g., "production/api/latency")
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TagPath {
    /// The segments of the path (e.g., ["production", "api", "latency"])
    segments: Vec<String>,
}

impl TagPath {
    /// Create a new tag path from a string (e.g., "production/api/latency")
    pub fn parse(s: &str) -> Self {
        let segments: Vec<String> = s
            .trim()
            .trim_start_matches('#')
            .split('/')
            .filter(|s| !s.is_empty())
            .map(|s| s.trim().to_lowercase())
            .collect();
        Self { segments }
    }

    /// Check if this path is empty
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Get the number of segments
    pub fn depth(&self) -> usize {
        self.segments.len()
    }

    /// Get the full path as a string (e.g., "production/api/latency")
    pub fn as_str(&self) -> String {
        self.segments.join("/")
    }

    /// Get the display name (last segment)
    pub fn name(&self) -> &str {
        self.segments.last().map(|s| s.as_str()).unwrap_or("")
    }

    /// Get the parent path (e.g., "production/api" for "production/api/latency")
    pub fn parent(&self) -> Option<TagPath> {
        if self.segments.len() > 1 {
            Some(TagPath {
                segments: self.segments[..self.segments.len() - 1].to_vec(),
            })
        } else {
            None
        }
    }

    /// Check if this path starts with another path (is a descendant)
    pub fn starts_with(&self, prefix: &TagPath) -> bool {
        if prefix.segments.len() > self.segments.len() {
            return false;
        }
        self.segments
            .iter()
            .zip(prefix.segments.iter())
            .all(|(a, b)| a == b)
    }

    /// Get the segments
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// Create a child path by appending a segment
    pub fn child(&self, name: &str) -> TagPath {
        let mut segments = self.segments.clone();
        segments.push(name.to_lowercase());
        TagPath { segments }
    }
}

impl std::fmt::Display for TagPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A node in the tag tree
#[derive(Debug, Clone, Default)]
pub struct TagNode {
    /// Child nodes by name
    pub children: HashMap<String, TagNode>,
    /// Buffer IDs tagged at exactly this level
    pub buffer_ids: HashSet<u64>,
}

impl TagNode {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get or create a child node
    fn get_or_create_child(&mut self, name: &str) -> &mut TagNode {
        self.children.entry(name.to_string()).or_default()
    }

    /// Count total buffers in this node and all descendants
    pub fn total_buffer_count(&self) -> usize {
        let mut count = self.buffer_ids.len();
        for child in self.children.values() {
            count += child.total_buffer_count();
        }
        count
    }

    /// Collect all buffer IDs from this node and descendants
    pub fn collect_buffer_ids(&self) -> HashSet<u64> {
        let mut ids = self.buffer_ids.clone();
        for child in self.children.values() {
            ids.extend(child.collect_buffer_ids());
        }
        ids
    }
}

/// A tree structure for organizing tags hierarchically
#[derive(Debug, Clone, Default)]
pub struct TagTree {
    /// Root node (contains top-level tags like "production", "staging")
    root: TagNode,
}

impl TagTree {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a tag to a buffer
    pub fn add_tag(&mut self, buffer_id: u64, tag: &TagPath) {
        if tag.is_empty() {
            return;
        }

        let mut node = &mut self.root;
        for segment in tag.segments() {
            node = node.get_or_create_child(segment);
        }
        node.buffer_ids.insert(buffer_id);
    }

    /// Remove a tag from a buffer
    pub fn remove_tag(&mut self, buffer_id: u64, tag: &TagPath) {
        if tag.is_empty() {
            return;
        }

        if let Some(node) = self.get_node_mut(tag) {
            node.buffer_ids.remove(&buffer_id);
        }
        // Note: We don't clean up empty nodes for simplicity
    }

    /// Remove all tags for a buffer
    pub fn remove_buffer(&mut self, buffer_id: u64) {
        Self::remove_buffer_recursive(&mut self.root, buffer_id);
    }

    fn remove_buffer_recursive(node: &mut TagNode, buffer_id: u64) {
        node.buffer_ids.remove(&buffer_id);
        for child in node.children.values_mut() {
            Self::remove_buffer_recursive(child, buffer_id);
        }
    }

    /// Get a node by path (immutable)
    pub fn get_node(&self, path: &TagPath) -> Option<&TagNode> {
        if path.is_empty() {
            return Some(&self.root);
        }

        let mut node = &self.root;
        for segment in path.segments() {
            node = node.children.get(segment)?;
        }
        Some(node)
    }

    /// Get a node by path (mutable)
    fn get_node_mut(&mut self, path: &TagPath) -> Option<&mut TagNode> {
        if path.is_empty() {
            return Some(&mut self.root);
        }

        let mut node = &mut self.root;
        for segment in path.segments() {
            node = node.children.get_mut(segment)?;
        }
        Some(node)
    }

    /// Get all buffer IDs matching a tag path (including descendants)
    pub fn get_buffer_ids(&self, path: &TagPath) -> HashSet<u64> {
        if path.is_empty() {
            // Return all buffer IDs
            return self.root.collect_buffer_ids();
        }

        self.get_node(path)
            .map(|node| node.collect_buffer_ids())
            .unwrap_or_default()
    }

    /// Get all top-level tag names with their buffer counts
    pub fn top_level_tags(&self) -> Vec<(String, usize)> {
        self.root
            .children
            .iter()
            .map(|(name, node)| (name.clone(), node.total_buffer_count()))
            .collect()
    }

    /// Get child tags of a path with their buffer counts
    pub fn child_tags(&self, path: &TagPath) -> Vec<(String, usize)> {
        self.get_node(path)
            .map(|node| {
                node.children
                    .iter()
                    .map(|(name, child)| (name.clone(), child.total_buffer_count()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all unique tag paths in the tree
    pub fn all_paths(&self) -> Vec<TagPath> {
        let mut paths = Vec::new();
        Self::collect_paths(&self.root, &[], &mut paths);
        paths
    }

    fn collect_paths(node: &TagNode, current: &[String], paths: &mut Vec<TagPath>) {
        for (name, child) in &node.children {
            let mut path_segments = current.to_vec();
            path_segments.push(name.clone());
            paths.push(TagPath {
                segments: path_segments.clone(),
            });
            Self::collect_paths(child, &path_segments, paths);
        }
    }

    /// Get autocomplete suggestions for a partial path
    pub fn autocomplete(&self, partial: &str) -> Vec<String> {
        let path = TagPath::parse(partial);
        let mut suggestions = Vec::new();

        if path.is_empty() {
            // Suggest top-level tags
            for (name, count) in self.top_level_tags() {
                suggestions.push(format!("{name} ({count})"));
            }
        } else {
            // Check if we're completing the last segment or looking for children
            let parent = path.parent();
            let last_segment = path.name();

            let node = if let Some(ref parent_path) = parent {
                self.get_node(parent_path)
            } else {
                Some(&self.root)
            };

            if let Some(node) = node {
                // Suggest matching children
                for (name, child) in &node.children {
                    if name.starts_with(last_segment) {
                        let full_path = if let Some(ref parent_path) = parent {
                            format!("{parent_path}/{name}")
                        } else {
                            name.clone()
                        };
                        let count = child.total_buffer_count();
                        suggestions.push(format!("{full_path} ({count})"));
                    }
                }

                // If exact match exists, also suggest its children
                if let Some(exact_node) = node.children.get(last_segment) {
                    for (child_name, child) in &exact_node.children {
                        let full_path = format!("{path}/{child_name}");
                        let count = child.total_buffer_count();
                        suggestions.push(format!("{full_path} ({count})"));
                    }
                }
            }
        }

        suggestions.sort();
        suggestions
    }
}

/// Current tag filter state
#[derive(Debug, Clone, Default)]
pub struct TagFilter {
    /// Active filter path (None = show all)
    pub active: Option<TagPath>,
}

impl TagFilter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the active filter
    pub fn set(&mut self, path: Option<TagPath>) {
        self.active = path;
    }

    /// Clear the filter
    pub fn clear(&mut self) {
        self.active = None;
    }

    /// Check if a buffer matches the current filter
    pub fn matches(&self, buffer_tags: &[String]) -> bool {
        let Some(ref filter_path) = self.active else {
            return true; // No filter = match all
        };

        buffer_tags.iter().any(|tag| {
            let tag_path = TagPath::parse(tag);
            tag_path.starts_with(filter_path)
        })
    }

    /// Get the active filter as a display string
    pub fn display(&self) -> String {
        self.active
            .as_ref()
            .map(|p| format!("#{p}"))
            .unwrap_or_else(|| "all".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_path_parse() {
        let path = TagPath::parse("production/api/latency");
        assert_eq!(path.segments(), &["production", "api", "latency"]);
        assert_eq!(path.depth(), 3);
        assert_eq!(path.name(), "latency");
        assert_eq!(path.as_str(), "production/api/latency");
    }

    #[test]
    fn test_tag_path_parse_with_hash() {
        let path = TagPath::parse("#production/api");
        assert_eq!(path.segments(), &["production", "api"]);
    }

    #[test]
    fn test_tag_path_parent() {
        let path = TagPath::parse("production/api/latency");
        let parent = path.parent().unwrap();
        assert_eq!(parent.as_str(), "production/api");

        let grandparent = parent.parent().unwrap();
        assert_eq!(grandparent.as_str(), "production");

        assert!(grandparent.parent().is_none());
    }

    #[test]
    fn test_tag_path_starts_with() {
        let path = TagPath::parse("production/api/latency");
        let prefix = TagPath::parse("production/api");

        assert!(path.starts_with(&prefix));
        assert!(path.starts_with(&TagPath::parse("production")));
        assert!(!prefix.starts_with(&path));
    }

    #[test]
    fn test_tag_tree_add_and_get() {
        let mut tree = TagTree::new();

        tree.add_tag(1, &TagPath::parse("production/api"));
        tree.add_tag(2, &TagPath::parse("production/api"));
        tree.add_tag(3, &TagPath::parse("production/db"));
        tree.add_tag(4, &TagPath::parse("staging/api"));

        // Get exact path
        let api_ids = tree.get_buffer_ids(&TagPath::parse("production/api"));
        assert!(api_ids.contains(&1));
        assert!(api_ids.contains(&2));
        assert!(!api_ids.contains(&3));

        // Get parent path (includes descendants)
        let prod_ids = tree.get_buffer_ids(&TagPath::parse("production"));
        assert!(prod_ids.contains(&1));
        assert!(prod_ids.contains(&2));
        assert!(prod_ids.contains(&3));
        assert!(!prod_ids.contains(&4));
    }

    #[test]
    fn test_tag_tree_remove() {
        let mut tree = TagTree::new();

        tree.add_tag(1, &TagPath::parse("production/api"));
        tree.add_tag(1, &TagPath::parse("critical"));

        tree.remove_tag(1, &TagPath::parse("production/api"));

        let api_ids = tree.get_buffer_ids(&TagPath::parse("production/api"));
        assert!(!api_ids.contains(&1));

        let critical_ids = tree.get_buffer_ids(&TagPath::parse("critical"));
        assert!(critical_ids.contains(&1));
    }

    #[test]
    fn test_tag_filter_matches() {
        let mut filter = TagFilter::new();

        // No filter = match all
        assert!(filter.matches(&["production/api".to_string()]));
        assert!(filter.matches(&[]));

        // With filter
        filter.set(Some(TagPath::parse("production")));
        assert!(filter.matches(&["production/api".to_string()]));
        assert!(filter.matches(&["production".to_string()]));
        assert!(!filter.matches(&["staging/api".to_string()]));
        assert!(!filter.matches(&[]));
    }
}
