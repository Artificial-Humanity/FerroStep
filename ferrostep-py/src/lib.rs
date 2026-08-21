//! PyO3 bridge for `ferrostep-core`.
//!
//! The boundary speaks JSON strings both ways: workflow definitions and
//! snapshots come in as JSON, decisions go out as JSON. The pure-Python wrapper
//! in `python/ferrostep/__init__.py` turns those into dicts, so the bridge
//! stays free of pyo3 <-> serde conversion machinery.

use ferrostep_core::{Engine as CoreEngine, Snapshot, WorkflowDef};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

#[pyclass(frozen)]
pub struct Engine {
    inner: CoreEngine,
}

#[pymethods]
impl Engine {
    /// Parse and validate a workflow definition. Raises ValueError with the
    /// core's validation message on any structural defect.
    #[new]
    fn new(workflow_json: &str) -> PyResult<Self> {
        let def = WorkflowDef::from_json(workflow_json)
            .map_err(|e| PyValueError::new_err(format!("invalid workflow JSON: {e}")))?;
        let inner = CoreEngine::new(def)
            .map_err(|e| PyValueError::new_err(format!("invalid workflow: {e}")))?;
        Ok(Engine { inner })
    }

    fn authorize_json(&self, snapshot_json: &str, role: &str, to: &str) -> PyResult<String> {
        let snap = parse_snapshot(snapshot_json)?;
        let decision = self.inner.authorize(&snap, role, to);
        serde_json::to_string(&decision).map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Each available move as one object: the transition's own fields, plus a
    /// `decision` holding what `authorize` would answer for it. Flattened
    /// rather than passed through as a pair, because a tuple crossing this
    /// boundary arrives in Python as a bare list and every caller then indexes
    /// it by position.
    fn next_moves_json(&self, snapshot_json: &str, role: &str) -> PyResult<String> {
        let snap = parse_snapshot(snapshot_json)?;
        let moves: Vec<serde_json::Value> = self
            .inner
            .next_moves(&snap, role)
            .into_iter()
            .map(|(transition, decision)| {
                let mut value = serde_json::to_value(transition)?;
                let object = value.as_object_mut().expect("a transition serializes as an object");
                object.insert("decision".to_string(), serde_json::to_value(&decision)?);
                Ok(value)
            })
            .collect::<Result<_, serde_json::Error>>()
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        serde_json::to_string(&moves).map_err(|e| PyValueError::new_err(e.to_string()))
    }

    fn status_json(&self, snapshot_json: &str) -> PyResult<String> {
        let snap = parse_snapshot(snapshot_json)?;
        serde_json::to_string(&self.inner.status(&snap))
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// The validated definition, normalized, as JSON.
    fn workflow_json(&self) -> PyResult<String> {
        serde_json::to_string(self.inner.def()).map_err(|e| PyValueError::new_err(e.to_string()))
    }
}

fn parse_snapshot(json: &str) -> PyResult<Snapshot> {
    serde_json::from_str(json).map_err(|e| PyValueError::new_err(format!("invalid snapshot JSON: {e}")))
}

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Engine>()?;
    Ok(())
}
