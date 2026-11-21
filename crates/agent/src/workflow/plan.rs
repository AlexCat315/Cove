use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::JsonSchema;
use crate::checkpoint::ToolPolicy;

use super::WorkflowError;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PlanData")]
pub struct WorkflowPlan {
    #[serde(flatten)]
    data: PlanData,
    #[serde(skip)]
    indices: BTreeMap<String, usize>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct PlanData {
    input: JsonSchema,
    steps: Vec<WorkflowStep>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: String,
    pub dependencies: Vec<String>,
    pub uses_input: bool,
    pub action: StepAction,
    pub output: JsonSchema,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepAction {
    Agent {
        agent: String,
        instructions: String,
        format: ArtifactFormat,
        tool_policy: ToolPolicy,
    },
    Review,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactFormat {
    Text,
    Json,
}

/// The original workflow input, when opted in, and artifacts from direct
/// dependencies. Absent input and a declared JSON null are distinct.
#[derive(Debug, Serialize)]
pub struct StepInputs<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<&'a Value>,
    pub artifacts: BTreeMap<&'a str, &'a Value>,
}

impl WorkflowPlan {
    pub fn new(input: JsonSchema, steps: Vec<WorkflowStep>) -> Result<Self, WorkflowError> {
        PlanData { input, steps }.try_into()
    }

    pub fn input_schema(&self) -> &JsonSchema {
        &self.data.input
    }

    pub fn steps(&self) -> &[WorkflowStep] {
        &self.data.steps
    }

    pub(super) fn index(&self, step: &str) -> Result<usize, WorkflowError> {
        self.indices
            .get(step)
            .copied()
            .ok_or_else(|| WorkflowError::UnknownStep(step.to_owned()))
    }
}

impl TryFrom<PlanData> for WorkflowPlan {
    type Error = WorkflowError;

    fn try_from(data: PlanData) -> Result<Self, Self::Error> {
        if data.steps.is_empty() {
            return Err(WorkflowError::Invalid(
                "a workflow requires at least one step".into(),
            ));
        }
        let mut indices = BTreeMap::new();
        for (index, step) in data.steps.iter().enumerate() {
            if !crate::portable_storage_id(&step.id)
                || indices.insert(step.id.clone(), index).is_some()
            {
                return Err(WorkflowError::Invalid(format!(
                    "invalid or duplicate step ID {:?}",
                    step.id
                )));
            }
            if let StepAction::Agent {
                agent,
                instructions,
                ..
            } = &step.action
                && (!crate::portable_storage_id(agent) || instructions.trim().is_empty())
            {
                return Err(WorkflowError::Invalid(format!(
                    "step {:?} requires a portable agent ID and instructions",
                    step.id
                )));
            }
        }

        let mut outgoing = vec![Vec::new(); data.steps.len()];
        let mut remaining = Vec::with_capacity(data.steps.len());
        for (index, step) in data.steps.iter().enumerate() {
            let mut dependencies = BTreeSet::new();
            for dependency in &step.dependencies {
                let source = indices.get(dependency).ok_or_else(|| {
                    WorkflowError::Invalid(format!(
                        "step {:?} has unknown dependency {dependency:?}",
                        step.id
                    ))
                })?;
                if !dependencies.insert(source) {
                    return Err(WorkflowError::Invalid(format!(
                        "step {:?} repeats dependency {dependency:?}",
                        step.id
                    )));
                }
                outgoing[*source].push(index);
            }
            remaining.push(dependencies.len());
        }

        let mut ready: VecDeque<_> = remaining
            .iter()
            .enumerate()
            .filter_map(|(index, count)| (*count == 0).then_some(index))
            .collect();
        let mut visited = 0;
        while let Some(source) = ready.pop_front() {
            visited += 1;
            for &target in &outgoing[source] {
                remaining[target] -= 1;
                if remaining[target] == 0 {
                    ready.push_back(target);
                }
            }
        }
        if visited != data.steps.len() {
            return Err(WorkflowError::Invalid(
                "workflow dependencies contain a cycle".into(),
            ));
        }
        Ok(Self { data, indices })
    }
}
