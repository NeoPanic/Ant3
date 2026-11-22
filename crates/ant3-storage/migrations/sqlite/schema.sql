PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;
CREATE TABLE IF NOT EXISTS presets(name TEXT NOT NULL,version TEXT NOT NULL,body TEXT NOT NULL,created_at INTEGER NOT NULL,PRIMARY KEY(name,version));
CREATE TABLE IF NOT EXISTS plugins(id TEXT NOT NULL,version TEXT NOT NULL,body TEXT NOT NULL,digest TEXT NOT NULL,created_at INTEGER NOT NULL,PRIMARY KEY(id,version));
CREATE TABLE IF NOT EXISTS instances(id TEXT PRIMARY KEY,fleet_id TEXT NOT NULL,preset_name TEXT NOT NULL,preset_version TEXT NOT NULL,state TEXT NOT NULL,snapshot TEXT NOT NULL,created_at INTEGER NOT NULL,FOREIGN KEY(preset_name,preset_version) REFERENCES presets(name,version));
CREATE TABLE IF NOT EXISTS runs(id TEXT PRIMARY KEY,instance_id TEXT NOT NULL REFERENCES instances(id),state TEXT NOT NULL,cancel_requested INTEGER NOT NULL DEFAULT 0,input TEXT NOT NULL,report TEXT,created_at INTEGER NOT NULL,started_at INTEGER,finished_at INTEGER);
CREATE INDEX IF NOT EXISTS runs_state ON runs(state,created_at);
CREATE TABLE IF NOT EXISTS sessions(id TEXT PRIMARY KEY,body TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS commands(key TEXT PRIMARY KEY,fingerprint TEXT NOT NULL,response TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS events(seq INTEGER PRIMARY KEY AUTOINCREMENT,kind TEXT NOT NULL,subject TEXT NOT NULL,body TEXT NOT NULL,at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS instance_deadlines(instance_id TEXT PRIMARY KEY REFERENCES instances(id),expires_at INTEGER NOT NULL);
CREATE INDEX IF NOT EXISTS instance_deadlines_expiry ON instance_deadlines(expires_at);
CREATE TABLE IF NOT EXISTS fleets(id TEXT PRIMARY KEY,preset_name TEXT NOT NULL,preset_version TEXT NOT NULL,snapshot TEXT NOT NULL,mode TEXT NOT NULL,desired_count INTEGER NOT NULL,state TEXT NOT NULL,revision INTEGER NOT NULL,ttl_seconds INTEGER,created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS fleet_slots(fleet_id TEXT NOT NULL REFERENCES fleets(id),slot INTEGER NOT NULL,generation INTEGER NOT NULL,enabled INTEGER NOT NULL,fulfilled INTEGER NOT NULL,instance_id TEXT REFERENCES instances(id),PRIMARY KEY(fleet_id,slot));
CREATE TABLE IF NOT EXISTS instance_allocations(instance_id TEXT PRIMARY KEY REFERENCES instances(id),fleet_id TEXT NOT NULL REFERENCES fleets(id),slot INTEGER NOT NULL,generation INTEGER NOT NULL,UNIQUE(fleet_id,slot,generation));
CREATE TABLE IF NOT EXISTS operations(id TEXT PRIMARY KEY,fleet_id TEXT NOT NULL REFERENCES fleets(id),kind TEXT NOT NULL,revision INTEGER NOT NULL,target_count INTEGER NOT NULL,state TEXT NOT NULL,created_at INTEGER NOT NULL,started_at INTEGER,finished_at INTEGER);
CREATE INDEX IF NOT EXISTS operations_fleet ON operations(fleet_id,created_at);
CREATE INDEX IF NOT EXISTS instances_fleet_state ON instances(fleet_id,state);
CREATE INDEX IF NOT EXISTS runs_instance_state ON runs(instance_id,state);
CREATE UNIQUE INDEX IF NOT EXISTS fleet_slots_instance ON fleet_slots(instance_id) WHERE instance_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS operations_active ON operations(fleet_id) WHERE state IN ('pending','running');
CREATE TABLE IF NOT EXISTS process_plugins(id TEXT NOT NULL,version TEXT NOT NULL,body TEXT NOT NULL,digest TEXT NOT NULL,state TEXT NOT NULL,health_at INTEGER,health_error TEXT,created_at INTEGER NOT NULL,PRIMARY KEY(id,version));
CREATE TABLE IF NOT EXISTS messages(seq INTEGER PRIMARY KEY AUTOINCREMENT,id TEXT NOT NULL UNIQUE,sender_kind TEXT NOT NULL,sender_instance_id TEXT REFERENCES instances(id),source_run_id TEXT REFERENCES runs(id),recipient_id TEXT NOT NULL REFERENCES instances(id),text TEXT NOT NULL,correlation_id TEXT NOT NULL,causation_id TEXT REFERENCES messages(id),depth INTEGER NOT NULL,created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS message_outbox(message_id TEXT PRIMARY KEY REFERENCES messages(id),state TEXT NOT NULL,attempts INTEGER NOT NULL DEFAULT 0,delivered_at INTEGER,error TEXT,resolution TEXT,resolved_at INTEGER);
CREATE TABLE IF NOT EXISTS message_inbox(message_id TEXT PRIMARY KEY REFERENCES messages(id),recipient_id TEXT NOT NULL REFERENCES instances(id),state TEXT NOT NULL,run_id TEXT UNIQUE REFERENCES runs(id),received_at INTEGER NOT NULL,ack_at INTEGER,finished_at INTEGER,error TEXT,resolution TEXT,resolved_at INTEGER);
CREATE INDEX IF NOT EXISTS message_outbox_state ON message_outbox(state,message_id);
CREATE INDEX IF NOT EXISTS message_inbox_recipient ON message_inbox(recipient_id,state);
CREATE INDEX IF NOT EXISTS messages_correlation ON messages(correlation_id,seq);
CREATE INDEX IF NOT EXISTS messages_recipient ON messages(recipient_id,seq);
CREATE INDEX IF NOT EXISTS messages_sender ON messages(sender_instance_id,seq);
CREATE INDEX IF NOT EXISTS messages_source_run ON messages(source_run_id);
CREATE TABLE IF NOT EXISTS workspace_artifacts(id TEXT PRIMARY KEY,instance_id TEXT NOT NULL REFERENCES instances(id),run_id TEXT NOT NULL REFERENCES runs(id),kind TEXT NOT NULL,name TEXT NOT NULL,path TEXT,media_type TEXT NOT NULL,content TEXT NOT NULL,digest TEXT NOT NULL,bytes INTEGER NOT NULL,created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS workspace_commands(run_id TEXT NOT NULL REFERENCES runs(id),command_key TEXT NOT NULL,fingerprint TEXT NOT NULL,response TEXT NOT NULL,PRIMARY KEY(run_id,command_key));
-- Operator-tunable runtime mechanics (see ant3_platform::settings).
CREATE TABLE IF NOT EXISTS settings(
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS model_providers (
    protocol TEXT PRIMARY KEY,
    model TEXT NOT NULL,
    base_url TEXT NOT NULL,
    api_key TEXT NOT NULL
);
