from pathlib import Path
import sqlite3,time
root=Path(__file__).resolve().parents[1]
schema=(root/'src-tauri/schema.sql').read_text(encoding='utf-8')
N=50000
con=sqlite3.connect(':memory:')
con.executescript(schema)
now=1760000000
con.executemany("INSERT INTO assets(path,name,created_at,updated_at) VALUES(?,?,?,?)",[(f'/tmp/{i}.png',f'asset {i}',now+i%100,now+i%100) for i in range(N)])
rows=con.execute('select id,name from assets').fetchall()
con.executemany("INSERT INTO prompt_state(asset_id,prompt,negative_prompt,model,updated_at) VALUES(?,?,?,?,?)",[(i,f'prompt blue character {i}','','GPT Image' if i%2 else 'Flux',now) for i,_ in rows])
con.executemany("INSERT INTO asset_search(asset_id,name,prompt,negative_prompt,model,tags) VALUES(?,?,?,?,?,?)",[(i,name,f'prompt blue character {i}','','GPT Image' if i%2 else 'Flux','') for i,name in rows])
con.executemany("INSERT INTO generation_index(asset_id,seed,steps,sampler,scheduler,cfg_scale,denoise) VALUES(?,?,?,?,?,?,?)",[
    (i,str(100000+i),20+i%31,'DPM++ 2M' if i%3 else 'Euler','karras' if i%2 else 'normal',3.0+(i%50)/10,0.5+(i%20)/100)
    for i,_ in rows
])
con.commit()
t=time.perf_counter()
page=con.execute("SELECT asset_id FROM asset_search WHERE asset_search MATCH ? LIMIT 240 OFFSET 4800",('"blue"*',)).fetchall()
fts_elapsed=time.perf_counter()-t
assert len(page)==240
t=time.perf_counter()
advanced=con.execute("""
    SELECT a.id FROM assets a
    JOIN generation_index gi ON gi.asset_id=a.id
    WHERE gi.sampler=? COLLATE NOCASE AND gi.steps BETWEEN ? AND ? AND gi.cfg_scale BETWEEN ? AND ?
    ORDER BY a.updated_at DESC LIMIT 240
""",('DPM++ 2M',24,36,4.0,6.0)).fetchall()
advanced_elapsed=time.perf_counter()-t
assert advanced
assert advanced_elapsed < 1.0
con.close()
print(f'ImageLore 50k search smoke test: PASS (FTS {fts_elapsed:.4f}s, generation {advanced_elapsed:.4f}s)')
