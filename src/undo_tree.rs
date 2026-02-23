use serde::{Deserialize, Serialize};

pub type NodeId = usize;

#[derive(Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub text: String,
    pub cursor_line: usize,
    pub cursor_col: usize,
}

#[derive(Serialize, Deserialize)]
pub struct UndoNode {
    pub id: NodeId,
    pub snapshot: Snapshot,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize)]
pub struct UndoTree {
    pub nodes: Vec<UndoNode>,
    pub current: NodeId,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl UndoTree {
    pub fn new(snapshot: Snapshot) -> Self {
        let root = UndoNode {
            id: 0,
            snapshot,
            parent: None,
            children: Vec::new(),
            timestamp: now_secs(),
        };
        Self { nodes: vec![root], current: 0 }
    }

    pub fn push(&mut self, snapshot: Snapshot) -> NodeId {
        let id = self.nodes.len();
        let parent = self.current;
        self.nodes.push(UndoNode {
            id,
            snapshot,
            parent: Some(parent),
            children: Vec::new(),
            timestamp: now_secs(),
        });
        self.nodes[parent].children.push(id);
        self.current = id;
        id
    }

    pub fn undo(&mut self) -> Option<Snapshot> {
        let parent_id = self.nodes[self.current].parent?;
        self.current = parent_id;
        Some(self.nodes[parent_id].snapshot.clone())
    }

    pub fn redo(&mut self) -> Option<Snapshot> {
        let child_id = *self.nodes[self.current].children.last()?;
        self.current = child_id;
        Some(self.nodes[child_id].snapshot.clone())
    }

    pub fn jump_to(&mut self, id: NodeId) -> Option<Snapshot> {
        if id >= self.nodes.len() {
            return None;
        }
        self.current = id;
        Some(self.nodes[id].snapshot.clone())
    }

    pub fn reset(&mut self, snapshot: Snapshot) {
        self.nodes.clear();
        let root = UndoNode {
            id: 0,
            snapshot,
            parent: None,
            children: Vec::new(),
            timestamp: now_secs(),
        };
        self.nodes.push(root);
        self.current = 0;
    }
}
