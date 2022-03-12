use crate::*;
use near_sdk::serde_json::{json, Value};

#[near_bindgen]
impl Dupwork {
    pub fn available_tasks(&self, from_index: u64, limit: u64) -> Vec<(TaskId, WrappedTask)> {
        let tasks_id = self.tasks_recores.keys_as_vector();

        let from = if tasks_id.len() > (limit + from_index) {
            tasks_id.len() - limit - from_index
        } else {
            0
        };

        let to = if tasks_id.len() > from_index {
            tasks_id.len() - from_index
        } else {
            0
        };

        (from..to)
        // (std::cmp::max(from_index..std::cmp::min(from_index + limit, tasks_id.len() as u64))
            .map(|index| {
                let task_id = tasks_id.get(index as u64).unwrap();
                let task = self.tasks_recores.get(&task_id.clone()).unwrap();
                (task_id.clone(), WrappedTask::from(task))
            })
            // .filter(|(_k, v)| {
            //     let available_until: u64 = v.available_until.into();
            //     (v.max_participants as u64 > v.proposals.len() as u64
            //         && available_until > env::block_timestamp())
            // })
            // .map(|(k, task)| (k, WrappedTask::from(task)))
            .rev()
            .collect()
    }

    pub fn current_tasks(
        &self,
        account_id: ValidAccountId,
        from_index: u64,
        limit: u64,
    ) -> Vec<(TaskId, WrappedTask)> {
        let tasks_id = self
            .users
            .get(&account_id)
            .expect("User not found")
            .current_jobs;

        tasks_id
            .iter()
            .map(|k| {
                (
                    k.clone(),
                    WrappedTask::from(self.tasks_recores.get(&k).unwrap()),
                )
            })
            .filter(|(_k, v)| v.proposals.len() > 0)
            .map(|(k, task)| (k, WrappedTask::from(task)))
            .collect()
    }

    pub fn completed_tasks(
        &self,
        account_id: ValidAccountId,
        from_index: u64,
        limit: u64,
    ) -> Vec<(TaskId, WrappedTask)> {
        let tasks_id = self
            .users
            .get(&account_id)
            .expect("User not found")
            .completed_jobs;

        tasks_id
            .iter()
            .map(|k| {
                (
                    k.clone(),
                    WrappedTask::from(self.tasks_recores.get(&k).unwrap()),
                )
            })
            .collect()
    }

    pub fn user_info(&self, account_id: ValidAccountId) -> Value {
        self.users
            .get(&account_id)
            .map(|v| {
                json!({
                    "account_id": v.account_id,
                    "user_type": WrappedUserType::from(v.user_type),
                    "completed_jobs": v.completed_jobs.to_vec()
                })
            })
            .expect("Canot map user to json")
    }

    pub fn task_by_id(&self, task_id: TaskId) -> WrappedTask {
        self.tasks_recores
            .get(&task_id)
            .map(|v| WrappedTask::from(v))
            .expect("Task not found")
    }
}
