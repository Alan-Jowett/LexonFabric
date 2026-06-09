use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use lexongraph_block::{Block, BlockHash, BranchEntry, EmbeddingSpec};
use lexongraph_block_store::{BlockStore, BlockStoreError};
use serde::Serialize;
use thiserror::Error;

use crate::tree_tools::decode_embedding_values;

#[derive(Debug, Error)]
pub enum TreeQualityError {
    #[error("root block {root_id} was not found")]
    MissingRootBlock { root_id: String },
    #[error("block {block_id} uses unsupported embedding spec {encoding}/{dims}")]
    UnsupportedEmbeddingSpec {
        block_id: String,
        encoding: String,
        dims: u64,
    },
    #[error("block {block_id} contains a non-finite embedding value")]
    NonFiniteEmbedding { block_id: String },
    #[error(transparent)]
    BlockStore(#[from] BlockStoreError),
    #[error("failed to render tree quality report")]
    Render(#[from] serde_json::Error),
    #[error("failed to write tree quality report {path}: {source}")]
    WriteArtifact {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum FindingSeverity {
    Error,
    Warning,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FindingKind {
    MissingChildBlock,
    ChildLevelNotLowerThanParent,
    EmbeddingSpecMismatch,
    CycleDetected,
    SharedChildReference,
    ChildSpreadExceedsParent,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct SpreadMetrics {
    pub centroid: Vec<f32>,
    pub mean_centroid_distance: f32,
    pub max_centroid_distance: f32,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct EmbeddingSpecReport {
    pub dims: u64,
    pub encoding: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct BlockQualityMetrics {
    pub block_id: String,
    pub kind: String,
    pub level: u64,
    pub entry_count: usize,
    pub parent_block_id: Option<String>,
    pub reachable_depth: usize,
    pub embedding_spec: EmbeddingSpecReport,
    pub spread: SpreadMetrics,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct TreeQualityFinding {
    pub severity: FindingSeverity,
    pub kind: FindingKind,
    pub block_id: String,
    pub parent_block_id: Option<String>,
    pub message: String,
    pub parent_mean_centroid_distance: Option<f32>,
    pub child_mean_centroid_distance: Option<f32>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct TreeQualitySummary {
    pub block_count: usize,
    pub branch_count: usize,
    pub leaf_count: usize,
    pub edge_count: usize,
    pub max_depth: usize,
    pub structural_finding_count: usize,
    pub quality_warning_count: usize,
    pub mean_block_mean_centroid_distance: f32,
    pub max_block_max_centroid_distance: f32,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct TreeQualityReport {
    pub root_id: String,
    pub summary: TreeQualitySummary,
    pub findings: Vec<TreeQualityFinding>,
    pub blocks: Vec<BlockQualityMetrics>,
}

#[derive(Clone, Debug)]
struct TraversalState {
    blocks: Vec<BlockQualityMetrics>,
    findings: Vec<TreeQualityFinding>,
    metrics_by_id: HashMap<BlockHash, BlockQualityMetrics>,
    visited: HashSet<BlockHash>,
    structural_finding_count: usize,
    edge_count: usize,
    max_depth: usize,
}

impl TraversalState {
    fn push_finding(&mut self, finding: TreeQualityFinding) {
        if finding.severity == FindingSeverity::Error {
            self.structural_finding_count += 1;
        }
        self.findings.push(finding);
    }
}

pub fn assess_rooted_tree(
    root_id: &BlockHash,
    store: &dyn BlockStore,
) -> Result<TreeQualityReport, TreeQualityError> {
    let Some(root) = store.get(root_id)? else {
        return Err(TreeQualityError::MissingRootBlock {
            root_id: root_id.to_string(),
        });
    };

    let mut state = TraversalState {
        blocks: Vec::new(),
        findings: Vec::new(),
        metrics_by_id: HashMap::new(),
        visited: HashSet::new(),
        structural_finding_count: 0,
        edge_count: 0,
        max_depth: 0,
    };
    let mut ancestry = Vec::new();
    traverse_block(
        root.hash,
        &root.block,
        None,
        0,
        store,
        &mut ancestry,
        &mut state,
    )?;
    state
        .blocks
        .sort_by(|left, right| left.block_id.cmp(&right.block_id));
    state.findings.sort_by(|left, right| {
        left.severity
            .cmp(&right.severity)
            .then_with(|| left.block_id.cmp(&right.block_id))
            .then_with(|| left.message.cmp(&right.message))
    });

    let block_count = state.blocks.len();
    let branch_count = state
        .blocks
        .iter()
        .filter(|block| block.kind == "branch")
        .count();
    let leaf_count = block_count - branch_count;
    let mean_block_mean_centroid_distance = if block_count == 0 {
        0.0
    } else {
        state
            .blocks
            .iter()
            .map(|block| block.spread.mean_centroid_distance)
            .sum::<f32>()
            / block_count as f32
    };
    let max_block_max_centroid_distance = state
        .blocks
        .iter()
        .map(|block| block.spread.max_centroid_distance)
        .fold(0.0f32, f32::max);
    let quality_warning_count = state
        .findings
        .iter()
        .filter(|finding| finding.severity == FindingSeverity::Warning)
        .count();

    Ok(TreeQualityReport {
        root_id: root_id.to_string(),
        summary: TreeQualitySummary {
            block_count,
            branch_count,
            leaf_count,
            edge_count: state.edge_count,
            max_depth: state.max_depth,
            structural_finding_count: state.structural_finding_count,
            quality_warning_count,
            mean_block_mean_centroid_distance,
            max_block_max_centroid_distance,
        },
        findings: state.findings,
        blocks: state.blocks,
    })
}

pub fn default_report_path(root_id: &BlockHash) -> PathBuf {
    PathBuf::from(format!(
        "block-tree-quality-{}.json",
        &root_id.to_string()[..8]
    ))
}

pub fn write_report(path: &Path, report: &TreeQualityReport) -> Result<(), TreeQualityError> {
    let rendered = serde_json::to_vec_pretty(report)?;
    fs::write(path, rendered).map_err(|source| TreeQualityError::WriteArtifact {
        path: path.display().to_string(),
        source,
    })
}

pub fn render_report_summary(report: &TreeQualityReport) -> String {
    let block_mean_by_id = report
        .blocks
        .iter()
        .map(|block| (block.block_id.as_str(), block.spread.mean_centroid_distance))
        .collect::<HashMap<_, _>>();
    let mut lines = vec![
        format!("Block-tree quality report for {}", report.root_id),
        format!(
            "Blocks: {} total ({} branch, {} leaf), {} edge(s), max depth {}, structural finding(s) {}, quality warning(s) {}",
            report.summary.block_count,
            report.summary.branch_count,
            report.summary.leaf_count,
            report.summary.edge_count,
            report.summary.max_depth,
            report.summary.structural_finding_count,
            report.summary.quality_warning_count
        ),
        format!(
            "Aggregate spread: mean block mean-centroid-distance {:.6}, max block max-centroid-distance {:.6}",
            report.summary.mean_block_mean_centroid_distance,
            report.summary.max_block_max_centroid_distance
        ),
        "Per-block spread:".into(),
    ];
    for block in &report.blocks {
        lines.push(format!(
            "- {} [{} level {} depth {} entries {} parent {}] mean {:.6}, max {:.6}",
            block.block_id,
            block.kind,
            block.level,
            block.reachable_depth,
            block.entry_count,
            block.parent_block_id.as_deref().unwrap_or("<root>"),
            block.spread.mean_centroid_distance,
            block.spread.max_centroid_distance
        ));
    }
    let mut comparisons = report
        .blocks
        .iter()
        .filter_map(|block| {
            let parent_id = block.parent_block_id.as_deref()?;
            let parent_mean = block_mean_by_id.get(parent_id).copied()?;
            Some((
                parent_id.to_string(),
                block.block_id.clone(),
                parent_mean,
                block.spread.mean_centroid_distance,
            ))
        })
        .collect::<Vec<_>>();
    comparisons.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    if !comparisons.is_empty() {
        lines.push("Parent/child centroid-distance comparisons:".into());
        for (parent_id, child_id, parent_mean, child_mean) in comparisons {
            let relation = if child_mean <= parent_mean {
                "child<=parent"
            } else {
                "child>parent"
            };
            lines.push(format!(
                "- parent {} mean {:.6} vs child {} mean {:.6} => {}",
                parent_id, parent_mean, child_id, child_mean, relation
            ));
        }
    }
    if !report.findings.is_empty() {
        lines.push("Findings:".into());
        for finding in &report.findings {
            lines.push(format!(
                "- {:?} {:?}: {}",
                finding.severity, finding.kind, finding.message
            ));
        }
    }
    lines.join("\n")
}

fn traverse_block(
    block_id: BlockHash,
    block: &Block,
    parent: Option<(BlockHash, &BlockQualityMetrics)>,
    depth: usize,
    store: &dyn BlockStore,
    ancestry: &mut Vec<BlockHash>,
    state: &mut TraversalState,
) -> Result<(), TreeQualityError> {
    if state.visited.contains(&block_id) {
        return Ok(());
    }
    state.max_depth = state.max_depth.max(depth);
    state.visited.insert(block_id);
    ancestry.push(block_id);

    let metrics = block_metrics(block_id, block, parent.as_ref().map(|(id, _)| *id), depth)?;
    if let Some((parent_id, parent_metrics)) = parent {
        if metrics.level >= parent_metrics.level {
            state.push_finding(TreeQualityFinding {
                severity: FindingSeverity::Error,
                kind: FindingKind::ChildLevelNotLowerThanParent,
                block_id: block_id.to_string(),
                parent_block_id: Some(parent_id.to_string()),
                message: format!(
                    "child {} level {} is not lower than parent {} level {}",
                    block_id, metrics.level, parent_id, parent_metrics.level
                ),
                parent_mean_centroid_distance: Some(parent_metrics.spread.mean_centroid_distance),
                child_mean_centroid_distance: Some(metrics.spread.mean_centroid_distance),
            });
        }
        if metrics.embedding_spec != parent_metrics.embedding_spec {
            state.push_finding(TreeQualityFinding {
                severity: FindingSeverity::Error,
                kind: FindingKind::EmbeddingSpecMismatch,
                block_id: block_id.to_string(),
                parent_block_id: Some(parent_id.to_string()),
                message: format!(
                    "child {} embedding spec {}/{} does not match parent {} embedding spec {}/{}",
                    block_id,
                    metrics.embedding_spec.encoding,
                    metrics.embedding_spec.dims,
                    parent_id,
                    parent_metrics.embedding_spec.encoding,
                    parent_metrics.embedding_spec.dims
                ),
                parent_mean_centroid_distance: Some(parent_metrics.spread.mean_centroid_distance),
                child_mean_centroid_distance: Some(metrics.spread.mean_centroid_distance),
            });
        }
        if metrics.spread.mean_centroid_distance
            > parent_metrics.spread.mean_centroid_distance + f32::EPSILON
        {
            state.push_finding(TreeQualityFinding {
                severity: FindingSeverity::Warning,
                kind: FindingKind::ChildSpreadExceedsParent,
                block_id: block_id.to_string(),
                parent_block_id: Some(parent_id.to_string()),
                message: format!(
                    "child {} mean centroid-distance spread {:.6} exceeds parent {} spread {:.6}",
                    block_id,
                    metrics.spread.mean_centroid_distance,
                    parent_id,
                    parent_metrics.spread.mean_centroid_distance
                ),
                parent_mean_centroid_distance: Some(parent_metrics.spread.mean_centroid_distance),
                child_mean_centroid_distance: Some(metrics.spread.mean_centroid_distance),
            });
        }
    }

    state.metrics_by_id.insert(block_id, metrics.clone());
    state.blocks.push(metrics.clone());

    if let Block::Branch(branch) = block {
        for entry in &branch.entries {
            state.edge_count += 1;
            handle_child_entry(block_id, &metrics, entry, depth + 1, store, ancestry, state)?;
        }
    }

    ancestry.pop();
    Ok(())
}

fn handle_child_entry(
    parent_id: BlockHash,
    parent_metrics: &BlockQualityMetrics,
    entry: &BranchEntry,
    depth: usize,
    store: &dyn BlockStore,
    ancestry: &mut Vec<BlockHash>,
    state: &mut TraversalState,
) -> Result<(), TreeQualityError> {
    if ancestry.contains(&entry.child) {
        state.push_finding(TreeQualityFinding {
            severity: FindingSeverity::Error,
            kind: FindingKind::CycleDetected,
            block_id: entry.child.to_string(),
            parent_block_id: Some(parent_id.to_string()),
            message: format!(
                "child {} closes a reachable cycle from parent {}",
                entry.child, parent_id
            ),
            parent_mean_centroid_distance: Some(parent_metrics.spread.mean_centroid_distance),
            child_mean_centroid_distance: None,
        });
        return Ok(());
    }
    if state.visited.contains(&entry.child) {
        state.push_finding(TreeQualityFinding {
            severity: FindingSeverity::Error,
            kind: FindingKind::SharedChildReference,
            block_id: entry.child.to_string(),
            parent_block_id: Some(parent_id.to_string()),
            message: format!(
                "child {} is reachable from multiple parent paths, so the rooted snapshot is not a tree",
                entry.child
            ),
            parent_mean_centroid_distance: Some(parent_metrics.spread.mean_centroid_distance),
            child_mean_centroid_distance: state
                .metrics_by_id
                .get(&entry.child)
                .map(|metrics| metrics.spread.mean_centroid_distance),
        });
        return Ok(());
    }
    let Some(validated_child) = store.get(&entry.child)? else {
        state.push_finding(TreeQualityFinding {
            severity: FindingSeverity::Error,
            kind: FindingKind::MissingChildBlock,
            block_id: entry.child.to_string(),
            parent_block_id: Some(parent_id.to_string()),
            message: format!(
                "parent {} references missing child block {}",
                parent_id, entry.child
            ),
            parent_mean_centroid_distance: Some(parent_metrics.spread.mean_centroid_distance),
            child_mean_centroid_distance: None,
        });
        return Ok(());
    };

    traverse_block(
        validated_child.hash,
        &validated_child.block,
        Some((parent_id, parent_metrics)),
        depth,
        store,
        ancestry,
        state,
    )
}

fn block_metrics(
    block_id: BlockHash,
    block: &Block,
    parent_block_id: Option<BlockHash>,
    reachable_depth: usize,
) -> Result<BlockQualityMetrics, TreeQualityError> {
    let (kind, level, embedding_spec, entry_count, spread) = match block {
        Block::Branch(branch) => (
            "branch",
            branch.level,
            EmbeddingSpecReport {
                dims: branch.embedding_spec.dims,
                encoding: branch.embedding_spec.encoding.clone(),
            },
            branch.entries.len(),
            spread_metrics(
                block_id,
                &branch.embedding_spec,
                branch.entries.iter().map(|entry| &entry.embedding),
            )?,
        ),
        Block::Leaf(leaf) => (
            "leaf",
            leaf.level,
            EmbeddingSpecReport {
                dims: leaf.embedding_spec.dims,
                encoding: leaf.embedding_spec.encoding.clone(),
            },
            leaf.entries.len(),
            spread_metrics(
                block_id,
                &leaf.embedding_spec,
                leaf.entries.iter().map(|entry| &entry.embedding),
            )?,
        ),
    };

    Ok(BlockQualityMetrics {
        block_id: block_id.to_string(),
        kind: kind.into(),
        level,
        entry_count,
        parent_block_id: parent_block_id.map(|value| value.to_string()),
        reachable_depth,
        embedding_spec,
        spread,
    })
}

fn spread_metrics<'a, I>(
    block_id: BlockHash,
    embedding_spec: &EmbeddingSpec,
    embeddings: I,
) -> Result<SpreadMetrics, TreeQualityError>
where
    I: Iterator<Item = &'a Vec<u8>>,
{
    let mut decoded = Vec::new();
    for embedding in embeddings {
        let Some(values) = decode_embedding_values(embedding, embedding_spec) else {
            return Err(TreeQualityError::UnsupportedEmbeddingSpec {
                block_id: block_id.to_string(),
                encoding: embedding_spec.encoding.clone(),
                dims: embedding_spec.dims,
            });
        };
        if values.iter().any(|value| !value.is_finite()) {
            return Err(TreeQualityError::NonFiniteEmbedding {
                block_id: block_id.to_string(),
            });
        }
        decoded.push(values);
    }

    let dimension_count = usize::try_from(embedding_spec.dims).unwrap_or(0);
    let mut centroid = vec![0.0f32; dimension_count];
    if decoded.is_empty() {
        return Ok(SpreadMetrics {
            centroid,
            mean_centroid_distance: 0.0,
            max_centroid_distance: 0.0,
        });
    }
    for vector in &decoded {
        for (index, value) in vector.iter().enumerate() {
            centroid[index] += *value;
        }
    }
    for value in &mut centroid {
        *value /= decoded.len() as f32;
    }

    let distances = decoded
        .iter()
        .map(|vector| {
            vector
                .iter()
                .zip(centroid.iter())
                .map(|(value, center)| {
                    let delta = *value - *center;
                    delta * delta
                })
                .sum::<f32>()
                .sqrt()
        })
        .collect::<Vec<_>>();
    let mean_centroid_distance = distances.iter().sum::<f32>() / distances.len() as f32;
    let max_centroid_distance = distances.iter().copied().fold(0.0f32, f32::max);

    Ok(SpreadMetrics {
        centroid,
        mean_centroid_distance,
        max_centroid_distance,
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use lexongraph_block::{Block, BranchBlock, Content, LeafBlock, LeafEntry, VERSION_1};
    use lexongraph_block_store_fs::FilesystemBlockStore;

    #[test]
    fn assessment_reports_structural_and_quality_findings() {
        let dir = tempdir().unwrap();
        let store = FilesystemBlockStore::new(dir.path().join("blocks")).unwrap();

        let left_left = store.put(&leaf_block(0, &[1.0, 0.0])).unwrap();
        let left_right = store.put(&leaf_block(0, &[-1.0, 0.0])).unwrap();
        let right_left = store.put(&leaf_block(0, &[0.2, 0.0])).unwrap();
        let right_right = store.put(&leaf_block(0, &[-0.2, 0.0])).unwrap();

        let left_branch = store
            .put(&branch_block(
                1,
                vec![([1.0, 0.0], left_left), ([-1.0, 0.0], left_right)],
            ))
            .unwrap();
        let right_branch = store
            .put(&branch_block(
                2,
                vec![([0.2, 0.0], right_left), ([-0.2, 0.0], right_right)],
            ))
            .unwrap();
        let root = store
            .put(&branch_block(
                2,
                vec![([0.2, 0.0], left_branch), ([-0.2, 0.0], right_branch)],
            ))
            .unwrap();

        let report = assess_rooted_tree(&root, &store).unwrap();

        assert_eq!(report.summary.block_count, 7);
        assert_eq!(report.summary.structural_finding_count, 1);
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.kind == FindingKind::ChildLevelNotLowerThanParent)
        );
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.kind == FindingKind::ChildSpreadExceedsParent)
        );
        let rendered = render_report_summary(&report);
        assert!(rendered.contains("Per-block spread:"));
        assert!(rendered.contains("Parent/child centroid-distance comparisons:"));
        assert!(rendered.contains("child<=parent"));
        assert!(rendered.contains("child>parent"));
    }

    #[test]
    fn assessment_writes_json_artifact() {
        let dir = tempdir().unwrap();
        let store = FilesystemBlockStore::new(dir.path().join("blocks")).unwrap();
        let root = store.put(&leaf_block(0, &[1.0, 0.0])).unwrap();

        let report = assess_rooted_tree(&root, &store).unwrap();
        let path = dir.path().join("report.json");
        write_report(&path, &report).unwrap();

        let rendered = fs::read_to_string(path).unwrap();
        assert!(rendered.contains("\"root_id\""));
    }

    fn branch_block(level: u64, entries: Vec<([f32; 2], BlockHash)>) -> Block {
        Block::Branch(BranchBlock {
            version: VERSION_1,
            level,
            embedding_spec: EmbeddingSpec {
                dims: 2,
                encoding: "f32le".into(),
            },
            entries: entries
                .into_iter()
                .map(|(embedding, child)| BranchEntry {
                    embedding: encode_f32(&embedding),
                    child,
                })
                .collect(),
            ext: None,
        })
    }

    fn leaf_block(level: u64, embedding: &[f32; 2]) -> Block {
        Block::Leaf(LeafBlock {
            version: VERSION_1,
            level,
            embedding_spec: EmbeddingSpec {
                dims: 2,
                encoding: "f32le".into(),
            },
            entries: vec![LeafEntry {
                embedding: encode_f32(embedding),
                metadata: Vec::new(),
                content: Content {
                    media_type: "text/plain".into(),
                    body: b"body".to_vec(),
                },
            }],
            ext: None,
        })
    }

    fn encode_f32(values: &[f32; 2]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    }
}
