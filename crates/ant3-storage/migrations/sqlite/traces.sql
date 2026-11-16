CREATE TABLE IF NOT EXISTS board_traces(
 root_id TEXT NOT NULL REFERENCES board_tasks(id),
 key TEXT NOT NULL,
 kind TEXT NOT NULL,
 strength REAL NOT NULL,
 deposits INTEGER NOT NULL DEFAULT 0,
 created_at INTEGER NOT NULL,
 updated_at INTEGER NOT NULL,
 PRIMARY KEY(root_id,key)
);
CREATE TABLE IF NOT EXISTS board_trace_deposits(
 seq INTEGER PRIMARY KEY AUTOINCREMENT,
 root_id TEXT NOT NULL REFERENCES board_tasks(id),
 key TEXT NOT NULL,
 amount REAL NOT NULL,
 reason TEXT NOT NULL,
 source_run_id TEXT REFERENCES runs(id),
 created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS board_trace_deposits_root ON board_trace_deposits(root_id,seq);
CREATE UNIQUE INDEX IF NOT EXISTS board_trace_reuse_once ON board_trace_deposits(root_id,key,source_run_id) WHERE reason='reuse';
CREATE UNIQUE INDEX IF NOT EXISTS board_trace_recruit_once ON board_trace_deposits(root_id,key,source_run_id) WHERE reason='recruitment';
CREATE TABLE IF NOT EXISTS board_task_retries(
 task_id TEXT PRIMARY KEY REFERENCES board_tasks(id),
 attempts INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS preset_experience(
 preset_name TEXT NOT NULL,
 preset_version TEXT NOT NULL,
 successes INTEGER NOT NULL DEFAULT 0,
 failures INTEGER NOT NULL DEFAULT 0,
 energy_spent INTEGER NOT NULL DEFAULT 0,
 PRIMARY KEY(preset_name,preset_version)
);CREATE TABLE IF NOT EXISTS board_verifications(
 id TEXT PRIMARY KEY,
 task_id TEXT NOT NULL REFERENCES board_tasks(id),
 attempt INTEGER NOT NULL,
 state TEXT NOT NULL,
 preset_name TEXT NOT NULL,
 preset_version TEXT NOT NULL,
 max_auto_reworks INTEGER NOT NULL,
 evidence TEXT NOT NULL,
 evidence_digest TEXT NOT NULL,
 acceptance TEXT NOT NULL,
 fleet_id TEXT REFERENCES fleets(id),
 instance_id TEXT REFERENCES instances(id),
 run_id TEXT UNIQUE REFERENCES runs(id),
 verdict TEXT,
 auto_rework INTEGER NOT NULL DEFAULT 0,
 error TEXT,
 created_at INTEGER NOT NULL,
 finished_at INTEGER,
 expires_at INTEGER NOT NULL,
 UNIQUE(task_id,attempt)
);
CREATE INDEX IF NOT EXISTS board_verifications_active ON board_verifications(state,task_id);
CREATE TABLE IF NOT EXISTS board_verification_reads(
 verification_id TEXT NOT NULL REFERENCES board_verifications(id),
 start_pos INTEGER NOT NULL,
 end_pos INTEGER NOT NULL,
 PRIMARY KEY(verification_id,start_pos,end_pos)
);
CREATE TABLE IF NOT EXISTS board_peer_roots(
 instance_id TEXT NOT NULL REFERENCES instances(id),
 root_id TEXT NOT NULL REFERENCES board_tasks(id),
 via_message TEXT NOT NULL REFERENCES messages(id),
 created_at INTEGER NOT NULL,
 PRIMARY KEY(instance_id,root_id)
);
CREATE TABLE IF NOT EXISTS board_energy(
 root_id TEXT PRIMARY KEY REFERENCES board_tasks(id),
 budget INTEGER NOT NULL,
 reserved INTEGER NOT NULL DEFAULT 0,
 spent INTEGER NOT NULL DEFAULT 0,
 refunded INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS energy_ledger(
 seq INTEGER PRIMARY KEY AUTOINCREMENT,
 root_id TEXT NOT NULL REFERENCES board_tasks(id),
 run_id TEXT REFERENCES runs(id),
 kind TEXT NOT NULL,
 amount INTEGER NOT NULL,
 balance INTEGER NOT NULL,
 created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS energy_ledger_root ON energy_ledger(root_id,seq);
CREATE UNIQUE INDEX IF NOT EXISTS energy_settle_once ON energy_ledger(run_id) WHERE kind IN ('settle','settle_unknown');
CREATE TABLE IF NOT EXISTS board_rework_chains(
 task_id TEXT PRIMARY KEY REFERENCES board_tasks(id),
 chain INTEGER NOT NULL,
 not_before INTEGER NOT NULL
);
