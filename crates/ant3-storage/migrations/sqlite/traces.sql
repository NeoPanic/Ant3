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