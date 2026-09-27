from pathlib import Path
import sqlite3, json
root=Path(__file__).resolve().parents[1]
schema=(root/'src-tauri/schema.sql').read_text(encoding='utf-8')
con=sqlite3.connect(':memory:')
con.executescript(schema)
now=1760000000
con.execute("INSERT INTO assets(path,name,created_at,updated_at) VALUES(?,?,?,?)",('x.png','Blue Character.png',now,now))
aid=con.execute('select id from assets').fetchone()[0]
con.execute("INSERT INTO prompt_state(asset_id,prompt,negative_prompt,model,updated_at) VALUES(?,?,?,?,?)",(aid,'blue hair marine character','low quality','GPT Image',now))
con.execute("INSERT INTO tags(name,created_at) VALUES(?,?)",('character',now))
tid=con.execute('select id from tags').fetchone()[0]
con.execute("INSERT INTO asset_tags(asset_id,tag_id,created_at) VALUES(?,?,?)",(aid,tid,now))
con.execute("INSERT INTO asset_search(asset_id,name,prompt,negative_prompt,model,tags) VALUES(?,?,?,?,?,?)",(aid,'Blue Character.png','blue hair marine character','low quality','GPT Image','character'))
hit=con.execute("SELECT asset_id FROM asset_search WHERE asset_search MATCH ?",('"blue"*',)).fetchone()
assert hit and hit[0]==aid
con.execute("INSERT INTO collections(name,description,created_at,updated_at) VALUES(?,?,?,?)",('Study','',now,now))
cid=con.execute('select id from collections').fetchone()[0]
con.execute("INSERT INTO collection_assets(collection_id,asset_id,created_at) VALUES(?,?,?)",(cid,aid,now))
con.execute("INSERT INTO prompt_revisions(asset_id,prompt,negative_prompt,model,tags_json,note,created_at) VALUES(?,?,?,?,?,?,?)",(aid,'p','n','m',json.dumps(['character']),'',now))
con.commit()
assert con.execute("select value from app_meta where key='schema_version'").fetchone()[0]=='1'
con.close()
print('ImageLore fresh schema: PASS')
