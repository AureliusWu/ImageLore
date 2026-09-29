from pathlib import Path
import sqlite3, json
root=Path(__file__).resolve().parents[1]
schema=(root/'src-tauri/schema.sql').read_text(encoding='utf-8')
con=sqlite3.connect(':memory:')
con.executescript(schema)
now=1760000000
con.execute("INSERT INTO assets(path,name,portable_id,created_at,updated_at) VALUES(?,?,?,?,?)",('x.png','Blue Character.png','il-schema-smoke',now,now))
aid=con.execute('select id from assets').fetchone()[0]
con.execute("INSERT INTO prompt_state(asset_id,prompt,negative_prompt,model,updated_at) VALUES(?,?,?,?,?)",(aid,'blue hair marine character','low quality','GPT Image',now))
con.execute("INSERT INTO tags(name,created_at) VALUES(?,?)",('character',now))
tid=con.execute('select id from tags').fetchone()[0]
con.execute("INSERT INTO asset_tags(asset_id,tag_id,created_at) VALUES(?,?,?)",(aid,tid,now))
con.execute("INSERT INTO visual_dna(asset_id,subject,lighting,environment,style,search_text,source,updated_at) VALUES(?,?,?,?,?,?,?,?)",(aid,'adult blue-haired character','soft window light','millennium computer room','photoreal editorial','adult blue-haired character soft window light millennium computer room photoreal editorial','manual',now))
con.execute("INSERT INTO asset_search(asset_id,name,prompt,negative_prompt,model,tags,visual_dna,reference) VALUES(?,?,?,?,?,?,?,?)",(aid,'Blue Character.png','blue hair marine character','low quality','GPT Image','character','adult blue-haired character soft window light millennium computer room photoreal editorial','PromptsRef inspiration browser-extension'))
hit=con.execute("SELECT asset_id FROM asset_search WHERE asset_search MATCH ?",('"blue"*',)).fetchone()
assert hit and hit[0]==aid
con.execute("INSERT INTO asset_cjk_search(rowid,asset_id,text) VALUES(?,?,?)",(aid,aid,'蓝色大肥鱼 夜晚卧室 海洋少女'))
cjk_hit=con.execute("SELECT asset_id FROM asset_cjk_search WHERE asset_cjk_search MATCH ?",('"蓝色大肥鱼"',)).fetchone()
assert cjk_hit and cjk_hit[0]==aid
con.execute("INSERT INTO collections(name,description,created_at,updated_at) VALUES(?,?,?,?)",('Study','',now,now))
cid=con.execute('select id from collections').fetchone()[0]
con.execute("INSERT INTO collection_assets(collection_id,asset_id,created_at) VALUES(?,?,?)",(cid,aid,now))
con.execute("INSERT INTO prompt_revisions(asset_id,prompt,negative_prompt,model,tags_json,note,created_at) VALUES(?,?,?,?,?,?,?)",(aid,'p','n','m',json.dumps(['character']),'',now))
con.execute("INSERT INTO generation_sessions(name,note,created_at,updated_at) VALUES(?,?,?,?)",('Session A','',now,now))
sid=con.execute('select id from generation_sessions').fetchone()[0]
con.execute("INSERT INTO asset_sessions(asset_id,session_id,note,created_at,updated_at) VALUES(?,?,?,?,?)",(aid,sid,'branch note',now,now))
con.execute("INSERT INTO model_aliases(alias,canonical,created_at,updated_at) VALUES(?,?,?,?)",('flux-dev.safetensors','Flux.1 Dev',now,now))
con.execute("INSERT INTO saved_filters(name,filter_json,created_at,updated_at) VALUES(?,?,?,?)",('Flux','{"query":"","view":"all","model":"Flux.1 Dev"}',now,now))
con.execute("INSERT INTO source_folders(path,name,auto_sync,last_scan_at,created_at,updated_at) VALUES(?,?,?,?,?,?)",('D:/AI/outputs','outputs',1,0,now,now))
con.execute("INSERT INTO generation_index(asset_id,seed,steps,sampler,scheduler,cfg_scale,denoise) VALUES(?,?,?,?,?,?,?)",(aid,'4120039',28,'DPM++ 2M','karras',5.5,0.72))
con.execute("INSERT INTO semantic_embeddings(asset_id,model_id,dimensions,vector,fingerprint,indexed_at) VALUES(?,?,?,?,?,?)",(aid,'clip-vit-b32-qdrant-v1',2,b'12345678','schema-fp',now))
con.execute("UPDATE vision_settings SET model=?,updated_at=? WHERE id=1",('vision-model',now))
con.execute("INSERT INTO image_prompt_analyses(asset_id,provider,model,summary,prompt,visual_dna_json,created_at) VALUES(?,?,?,?,?,?,?)",(aid,'openai-compatible','vision-model','summary','generated prompt','{}',now))
con.execute("INSERT INTO reference_sources(asset_id,source_url,page_url,page_title,source_type,metadata_json,captured_at,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?)",(aid,'https://cdn.example/a.png','https://promptsref.com/post','PromptsRef inspiration','browser-extension','{"intent":"remix"}',now,now,now))
con.commit()
assert con.execute("select value from app_meta where key='schema_version'").fetchone()[0]=='11'
assert con.execute("select portable_id from assets where id=?",(aid,)).fetchone()[0]=='il-schema-smoke'
assert con.execute("select count(*) from generation_sessions").fetchone()[0]==1
assert con.execute("select count(*) from saved_filters").fetchone()[0]==1
assert con.execute("select auto_sync from source_folders").fetchone()[0]==1
assert con.execute("select seed from generation_index where asset_id=?",(aid,)).fetchone()[0]=='4120039'
assert con.execute("select model_id from semantic_embeddings where asset_id=?",(aid,)).fetchone()[0]=='clip-vit-b32-qdrant-v1'
assert con.execute("select value from semantic_settings where key='enabled'").fetchone()[0]=='0'
assert con.execute("select environment from visual_dna where asset_id=?",(aid,)).fetchone()[0]=='millennium computer room'
assert con.execute("select model from vision_settings where id=1").fetchone()[0]=='vision-model'
assert con.execute("select prompt from image_prompt_analyses where asset_id=?",(aid,)).fetchone()[0]=='generated prompt'
assert con.execute("select page_title from reference_sources where asset_id=?",(aid,)).fetchone()[0]=='PromptsRef inspiration'
dna_hit=con.execute("SELECT asset_id FROM asset_search WHERE asset_search MATCH ?",('"millennium"*',)).fetchone()
assert dna_hit and dna_hit[0]==aid
con.close()
print('ImageLore v11 fresh schema: PASS')
