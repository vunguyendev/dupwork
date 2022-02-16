use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedMap, UnorderedSet, Vector};
use near_sdk::json_types::{ValidAccountId, WrappedBalance};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{
    env, ext_contract, near_bindgen, setup_alloc, Balance, Duration, Gas, Promise, PromiseResult,
};
use std::convert::TryFrom;

pub use crate::constants::*;
pub use crate::json_types::*;
pub use crate::types::*;
pub use crate::views::*;

mod constants;
mod json_types;
mod types;
mod views;

setup_alloc!();

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Dupwork {
    tasks_recores: UnorderedMap<TaskId, Task>,
    users: LookupMap<ValidAccountId, User>,
}

#[ext_contract(ext_self)]
pub trait ExtDupwork {
    fn on_transferd(
        &mut self,
        task_id: String,
        beneficiary_id: ValidAccountId,
        amount_to_transfer: Balance,
    ) -> bool;
}

impl Default for Dupwork {
    fn default() -> Self {
        Self {
            tasks_recores: UnorderedMap::new(b"tasks_recores".to_vec()),
            users: LookupMap::new(b"users".to_vec()),
        }
    }
}

#[near_bindgen]
impl Dupwork {
    #[init]
    pub fn new() -> Self {
        assert!(!env::state_exists(), "The contract is already initialized",);

        Self {
            tasks_recores: UnorderedMap::new(b"tasks_recores".to_vec()),
            users: LookupMap::new(b"users".to_vec()),
        }
    }

    #[payable]
    pub fn register(&mut self, requester: bool) {
        assert!(
            env::attached_deposit() == REGISTER_BOND,
            "Send exactly {:?} Near to register",
            REGISTER_BOND
        );

        let account_id = ValidAccountId::try_from(env::predecessor_account_id()).unwrap();
        let mut current_jobs_prefix = Vec::with_capacity(33);
        // Adding unique prefix.
        current_jobs_prefix.push(b'c');
        current_jobs_prefix.extend(env::sha256(env::predecessor_account_id().as_bytes()));

        let mut completed_jobs_prefix = Vec::with_capacity(33);
        // Adding unique prefix.
        completed_jobs_prefix.push(b'd');
        completed_jobs_prefix.extend(env::sha256(env::predecessor_account_id().as_bytes()));

        if requester {
            let user = User {
                account_id: account_id.clone(),
                user_type: UserType::Requester {
                    total_transfered: 0,
                    current_requests: 0,
                },
                completed_jobs: UnorderedSet::new(completed_jobs_prefix),
                current_jobs: UnorderedSet::new(current_jobs_prefix),
            };

            self.users.insert(&account_id, &user);
        } else {
            let user = User {
                account_id: account_id.clone(),
                user_type: UserType::Worker {
                    total_received: 0,
                    current_applies: 0,
                },
                completed_jobs: UnorderedSet::new(completed_jobs_prefix),
                current_jobs: UnorderedSet::new(current_jobs_prefix),
            };

            self.users.insert(&account_id, &user);
        }
    }

    pub fn leave(&mut self) {
        let account_id = ValidAccountId::try_from(env::predecessor_account_id()).unwrap();
        self.users
            .get(&account_id)
            .expect("You are not a member of dupwork");

        Promise::new(env::predecessor_account_id()).transfer(REGISTER_BOND);

        self.users.remove(&account_id);
    }

    /// Requester sections:
    pub fn new_task(
        &mut self,
        title: String,
        description: String,
        price: WrappedBalance,
        max_participants: u16,
    ) {
        let owner = ValidAccountId::try_from(env::predecessor_account_id()).unwrap();
        let mut user = self
            .users
            .get(&owner)
            .expect("You are not a member of dupwork");

        match user.user_type {
            UserType::Worker { .. } => panic!("Only requester can create a task"),
            UserType::Requester {
                total_transfered,
                current_requests,
            } => {
                assert!(
                    description.len() <= MAXIMUM_DESCRIPTION_LENGTH,
                    "Description too long"
                );

                assert!(
                    max_participants <= MAXIMUM_PROPOSAL_AT_ONE_TIME,
                    "Only accept {} at one time",
                    MAXIMUM_PROPOSAL_AT_ONE_TIME
                );

                let task_id = env::predecessor_account_id() + "_" + &env::block_index().to_string();

                assert!(
                    !self.tasks_recores.get(&task_id).is_some(),
                    "Can't post twice per block"
                );

                let mut proposal_prefix = Vec::with_capacity(33);
                // Adding unique prefix.
                proposal_prefix.push(b'p');
                proposal_prefix.extend(env::sha256(task_id.as_bytes()));

                let task = Task {
                    owner: owner.clone(),
                    title,
                    description,
                    price: price.into(),
                    /// When SO try to apply, their offer has to be less than 20% different from owner estimation.
                    max_participants,
                    proposals: Vector::new(proposal_prefix),
                    status: JobStatus::ReadyForApply,
                };

                env::log(format!("New task: {:?}", task_id).as_bytes());

                self.tasks_recores.insert(&task_id, &task); 
                //Update user current requests
                user.user_type = UserType::Requester {
                    total_transfered,
                    current_requests: current_requests + 1,
                };
                user.current_jobs.insert(&task_id);
                self.users.insert(&owner, &user);
            }
        }
    }

    #[payable]
    pub fn select_proposal(&mut self, task_id: String, index: u64) {
        let mut task = self
            .tasks_recores
            .get(&task_id)
            .expect("Task does not exist");

        let owner_id = ValidAccountId::try_from(env::predecessor_account_id()).unwrap();
        assert!(task.owner == owner_id, "Only owner can select proposal");

        let selected_proposal = task.proposals.get(index).expect("Not found proposal");

        assert!(
            task.status == JobStatus::ReadyForApply,
            "Job status must be ReadyForApply"
        );

        assert!(
            env::attached_deposit() == selected_proposal.price,
            "Attach exactly {} yoctoNear",
            selected_proposal.price
        );

        let mut worker = self.users
            .get(&selected_proposal.account_id)
            .expect("User not found");
        
        if let UserType::Worker {
            total_received,
            current_applies,
        } = worker.user_type {
            worker.user_type = UserType::Worker {
                total_received,
                current_applies: current_applies + 1,
            };

            worker.current_jobs.insert(&task_id);
            self.users.insert(&selected_proposal.account_id, &worker);
        }

        // let mut owner = self.users
        //     .get(&owner_id)
        //     .expect("User not found");

        // if let UserType::Requester {
        //     total_transfered,
        //     current_requests,
        // } = owner.user_type {
        //     owner.user_type = UserType::Requester {
        //         total_transfered,
        //         current_requests: current_requests + 1,
        //     };
        //     self.users.insert(&task.owner, &owner);
        // }

        task.status = JobStatus::FoundWorker;
        task.proposals.clear();
        task.proposals.push(&selected_proposal);
        self.tasks_recores.insert(&task_id, &task);

        env::log(
            format!(
                "Task after modify: {:#?}",
                self.tasks_recores
                    .get(&task_id)
                    .expect("Task does not exist")
            )
            .as_bytes(),
        );
    }

    pub fn approve_work(&mut self, task_id: String) {
        let task = self.tasks_recores.get(&task_id).expect("Job not exist");

        let owner = ValidAccountId::try_from(env::predecessor_account_id()).unwrap();
        assert!(task.owner == owner, "Only owner can select proposal");

        assert!(
            task.status == JobStatus::WorkSubmitted,
            "Job status must be WorkSubmitted"
        );

        let proposal = task.proposals.get(0).expect("Not found proposal");
        let beneficiary_id = proposal.account_id;
        let amount_to_transfer = proposal.price.into();
        // Make a transfer to the worker
        Promise::new(beneficiary_id.to_string())
            .transfer(amount_to_transfer)
            .then(ext_self::on_transferd(
                task_id,
                beneficiary_id,
                amount_to_transfer,
                &env::current_account_id(),
                0,
                DEFAULT_GAS_TO_PAY,
            ));
    }

    pub fn reject_work(&mut self, task_id: String) {
        let mut task = self.tasks_recores.get(&task_id).expect("Job not exist");

        let owner = ValidAccountId::try_from(env::predecessor_account_id()).unwrap();
        assert!(task.owner == owner, "Only owner can select proposal");

        assert!(
            task.status == JobStatus::WorkSubmitted,
            "Job status must be WorkSubmitted"
        );

        task.status = JobStatus::Pending;
        self.tasks_recores.insert(&task_id, &task);
    }


    /// Worker sections:
    pub fn submit_proposal(
        &mut self,
        task_id: String,
        cover_letter: String,
        price: WrappedBalance,
    ) {
        let account_id = ValidAccountId::try_from(env::predecessor_account_id()).unwrap();
        self.users
            .get(&account_id)
            .expect("You are not a member of dupwork");

        assert!(
            cover_letter.len() <= MAXIMUM_COVER_LETTER_LENGTH,
            "Cover letter is too long"
        );

        // assert job not found
        let mut task = self
            .tasks_recores
            .get(&task_id)
            .expect("Task does not exist");
        let proposal = Proposal {
            account_id,
            cover_letter,
            price: price.into(),
            proof_of_work: "".to_string(),
        };

        //add proposal to job records
        assert!(
            task.proposals.len() as u16 <= task.max_participants,
            "Workers limit has been reached"
        );

        task.proposals.push(&proposal);
        self.tasks_recores.insert(&task_id, &task);
    }

    pub fn submit_work(&mut self, task_id: String, url: String) {
        let mut task = self.tasks_recores.get(&task_id).expect("Job not exist");

        let worker = ValidAccountId::try_from(env::predecessor_account_id()).unwrap();
        assert!(
            task.proposals.get(0).unwrap().account_id == worker,
            "Only worker can submit their work"
        );

        let mut proposal = task.proposals.get(0).unwrap();

        proposal.proof_of_work = url;
        task.proposals.replace(0, &proposal);
        task.status = JobStatus::WorkSubmitted;
        self.tasks_recores.insert(&task_id, &task);
    }

    // Ext
    pub fn on_transferd(
        &mut self,
        task_id: String,
        beneficiary_id: ValidAccountId,
        amount_to_transfer: Balance,
    ) -> bool {
        assert!(
            env::predecessor_account_id() == env::current_account_id(),
            "Callback is not called from the contract itself",
        );

        assert!(
            env::promise_results_count() == 1,
            "Function called not as a callback",
        );

        match env::promise_result(0) {
            PromiseResult::Successful(_) => {
                let mut task = self.tasks_recores.get(&task_id).expect("Job not exist");
                task.status = JobStatus::Payout;
                self.tasks_recores.insert(&task_id, &task);

                let mut worker = self.users.get(&beneficiary_id).expect("Not found worker");
                worker.completed_jobs.insert(&task_id);
                worker.current_jobs.remove(&task_id);

                if let UserType::Worker {
                    total_received,
                    current_applies,
                } = worker.user_type {

                    env::log(
                        format!("Worker = {} {}", total_received, current_applies)
                        .as_bytes()
                    );          
                    worker.user_type = UserType::Worker {
                        total_received: total_received + amount_to_transfer,
                        current_applies: current_applies - 1,
                    };

                    self.users.insert(&beneficiary_id, &worker);
                }

                let mut owner = self.users.get(&task.owner).expect("Not found owner");
                owner.completed_jobs.insert(&task_id);
                owner.current_jobs.remove(&task_id);
                if let UserType::Requester {
                    total_transfered,
                    current_requests,
                } = owner.user_type {
                    owner.user_type = UserType::Requester {
                        total_transfered: total_transfered + amount_to_transfer,
                        current_requests: current_requests - 1,
                    };

                    self.users.insert(&task.owner, &owner);
                }
                true
            }
            _ => false,
        }
    }
}
