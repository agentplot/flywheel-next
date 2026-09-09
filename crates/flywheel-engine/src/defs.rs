//! Machine definitions as data: the serde types for `schema.json`.
//! Nothing here names a store, a service, a path or a field of the world (I13).

use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize)]
pub struct Machine {
    pub machine: String,
    pub version: u32,
    pub kind: MachineKind,
    #[serde(default)]
    pub object: Option<String>,
    #[serde(default)]
    pub parent: Option<Value>,
    #[serde(default)]
    pub owns: Vec<String>,
    #[serde(default)]
    pub singleton: Option<String>,
    #[serde(default)]
    pub params: BTreeMap<String, String>,
    #[serde(default)]
    pub record: BTreeMap<String, Value>,
    #[serde(default)]
    pub doc: Option<String>,
    pub regions: BTreeMap<String, Region>,
    #[serde(default)]
    pub satisfies: Vec<u32>,
}

impl Machine {
    /// The owning object kinds, however `parent:` was written (string, list or null).
    pub fn parents(&self) -> Vec<String> {
        match &self.parent {
            Some(Value::String(s)) => vec![s.clone()],
            Some(Value::Array(a)) => a.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
            _ => vec![],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MachineKind {
    Object,
    Template,
    Engine,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Region {
    #[serde(default)]
    pub doc: Option<String>,
    pub initial: String,
    pub states: BTreeMap<String, State>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct State {
    #[serde(default)]
    pub doc: Option<String>,
    #[serde(default, rename = "final")]
    pub is_final: bool,
    #[serde(default)]
    pub decision: Option<Decision>,
    #[serde(default)]
    pub tail: Option<String>,
    #[serde(default)]
    pub entry: Vec<Effect>,
    #[serde(default)]
    pub exit: Vec<Effect>,
    /// A template or object machine run inside this state.
    #[serde(default)]
    pub machine: Option<String>,
    #[serde(default)]
    pub params: Option<BTreeMap<String, Value>>,
    #[serde(default)]
    pub regions: BTreeMap<String, Region>,
    #[serde(default)]
    pub enter: BTreeMap<String, String>,
    #[serde(default)]
    pub transitions: Vec<Transition>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Decision {
    pub kind: String,
    #[serde(default = "default_group")]
    pub group: String,
    #[serde(default)]
    pub answers: Vec<String>,
    #[serde(default)]
    pub shows: Vec<String>,
    #[serde(default)]
    pub batch: Option<String>,
    #[serde(default)]
    pub document: Option<String>,
    #[serde(default)]
    pub satisfies: Vec<u32>,
}

fn default_group() -> String {
    "decide".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct Effect {
    #[serde(rename = "do")]
    pub name: String,
    #[serde(default)]
    pub args: Option<BTreeMap<String, Value>>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Transition {
    pub when: Guard,
    pub to: String,
    #[serde(default)]
    pub effects: Vec<Effect>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub bump: Option<String>,
    #[serde(default)]
    pub enter: BTreeMap<String, String>,
}

/// The guard algebra: the whole of the engine's vocabulary.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Guard {
    All {
        all: Vec<Guard>,
    },
    Any {
        any: Vec<Guard>,
    },
    Not {
        not: Box<Guard>,
    },
    Ev(EvGuard),
    Response {
        response: String,
    },
    Final {
        #[serde(rename = "final")]
        final_state: String,
    },
    Children {
        children: ChildrenGuard,
    },
    Parent {
        parent: ParentGuard,
    },
    Region {
        region: RegionGuard,
    },
    Always {
        always: bool,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvGuard {
    pub ev: String,
    #[serde(default)]
    pub is: Option<Value>,
    #[serde(default, rename = "in")]
    pub in_: Option<Vec<Value>>,
    #[serde(default)]
    pub exists: Option<bool>,
    #[serde(default)]
    pub gt: Option<Value>,
    #[serde(default)]
    pub gte: Option<Value>,
    #[serde(default)]
    pub lt: Option<Value>,
    #[serde(default)]
    pub lte: Option<Value>,
    #[serde(default)]
    pub older: Option<String>,
    #[serde(default)]
    pub eq_ev: Option<String>,
    #[serde(default)]
    pub ne_ev: Option<String>,
    #[serde(default)]
    pub gte_ev: Option<String>,
    #[serde(default)]
    pub lt_ev: Option<String>,
    #[serde(default)]
    pub gt_ev: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChildrenGuard {
    pub kind: String,
    #[serde(default)]
    pub all: Option<Vec<String>>,
    #[serde(default)]
    pub any: Option<Vec<String>>,
    #[serde(default)]
    pub none: Option<Vec<String>>,
    #[serde(default)]
    pub count_gte: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ParentGuard {
    #[serde(rename = "in")]
    pub in_: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegionGuard {
    pub name: String,
    #[serde(rename = "in")]
    pub in_: Vec<String>,
}

/// The atoms file: evidence names and effects with their proofs. The engine
/// reads only the effect → proof map; it never interprets a name.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Atoms {
    #[serde(default)]
    pub evidence: BTreeMap<String, Value>,
    #[serde(default)]
    pub effects: BTreeMap<String, Value>,
}

impl Atoms {
    pub fn proof_of(&self, effect: &str) -> Option<String> {
        self.effects
            .get(effect)
            .and_then(|v| v.get("proof"))
            .and_then(|p| p.as_str())
            .map(String::from)
    }
}

/// Every machine the engine knows, by name.
#[derive(Debug, Clone, Default)]
pub struct Definitions {
    pub machines: BTreeMap<String, Machine>,
    pub atoms: Atoms,
}

impl Definitions {
    pub fn get(&self, name: &str) -> Option<&Machine> {
        self.machines.get(name)
    }

    /// The machine that governs an object kind.
    pub fn for_object(&self, kind: &str) -> Option<&Machine> {
        self.machines
            .values()
            .find(|m| m.kind != MachineKind::Template && m.object.as_deref() == Some(kind))
            .or_else(|| self.machines.get(kind))
    }
}

impl Machine {
    /// Every atom name this machine reads or acts by: the evidence its guards
    /// name and the effects its states and transitions call. The engine reads
    /// no meaning from either — this is how a set that carries a machine can
    /// carry the atoms it needs and no more.
    pub fn atom_names(&self) -> (std::collections::BTreeSet<String>, std::collections::BTreeSet<String>) {
        let mut evidence = std::collections::BTreeSet::new();
        let mut effects = std::collections::BTreeSet::new();
        for region in self.regions.values() {
            region.atom_names(&mut evidence, &mut effects);
        }
        (evidence, effects)
    }
}

impl Region {
    fn atom_names(
        &self,
        evidence: &mut std::collections::BTreeSet<String>,
        effects: &mut std::collections::BTreeSet<String>,
    ) {
        for state in self.states.values() {
            for effect in state.entry.iter().chain(&state.exit) {
                effects.insert(effect.name.clone());
            }
            for transition in &state.transitions {
                guard_names(&transition.when, evidence);
                for effect in &transition.effects {
                    effects.insert(effect.name.clone());
                }
            }
            for nested in state.regions.values() {
                nested.atom_names(evidence, effects);
            }
        }
    }
}

/// The evidence names a guard reads, however deeply it is nested.
fn guard_names(guard: &Guard, into: &mut std::collections::BTreeSet<String>) {
    match guard {
        Guard::All { all } => all.iter().for_each(|g| guard_names(g, into)),
        Guard::Any { any } => any.iter().for_each(|g| guard_names(g, into)),
        Guard::Not { not } => guard_names(not, into),
        Guard::Ev(e) => {
            into.insert(e.ev.clone());
            for other in [&e.eq_ev, &e.ne_ev, &e.gte_ev, &e.lt_ev, &e.gt_ev]
                .into_iter()
                .flatten()
            {
                into.insert(other.clone());
            }
        }
        Guard::Response { .. }
        | Guard::Final { .. }
        | Guard::Children { .. }
        | Guard::Parent { .. }
        | Guard::Region { .. }
        | Guard::Always { .. } => {}
    }
}
