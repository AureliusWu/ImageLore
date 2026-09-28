from pathlib import Path
import sqlite3,time,struct,math
root=Path(__file__).resolve().parents[1]
schema=(root/'src-tauri/schema.sql').read_text(encoding='utf-8')
N=50000
con=sqlite3.connect(':memory:')
con.executescript(schema)
now=1760000000
con.executemany("INSERT INTO assets(path,name,created_at,updated_at) VALUES(?,?,?,?)",[(f'/tmp/{i}.png',f'asset {i}',now+i%100,now+i%100) for i in range(N)])
rows=con.execute('select id,name from assets').fetchall()
con.executemany("INSERT INTO prompt_state(asset_id,prompt,negative_prompt,model,updated_at) VALUES(?,?,?,?,?)",[(i,f'prompt blue character 蓝发角色 {i}','','GPT Image' if i%2 else 'Flux',now) for i,_ in rows])
con.executemany("INSERT INTO asset_search(asset_id,name,prompt,negative_prompt,model,tags) VALUES(?,?,?,?,?,?)",[(i,name,f'prompt blue character 蓝发角色 {i}','','GPT Image' if i%2 else 'Flux','') for i,name in rows])
con.executemany("INSERT INTO asset_cjk_search(rowid,asset_id,text) VALUES(?,?,?)",[
    (i,i,f'{name} 蓝发角色 海洋少女 夜晚卧室 prompt {i}') for i,name in rows
])
con.executemany("INSERT INTO generation_index(asset_id,seed,steps,sampler,scheduler,cfg_scale,denoise) VALUES(?,?,?,?,?,?,?)",[
    (i,str(100000+i),20+i%31,'DPM++ 2M' if i%3 else 'Euler','karras' if i%2 else 'normal',3.0+(i%50)/10,0.5+(i%20)/100)
    for i,_ in rows
])
def semantic_blob(i):
    values=[((i*(j+3))%97)/97.0 for j in range(16)]
    norm=math.sqrt(sum(v*v for v in values)) or 1.0
    return struct.pack('<16f',*(v/norm for v in values))
con.executemany("INSERT INTO semantic_embeddings(asset_id,model_id,dimensions,vector,fingerprint,indexed_at) VALUES(?,?,?,?,?,?)",[
    (i,'clip-vit-b32-qdrant-v1',16,semantic_blob(i),f'fp-{i}',now) for i,_ in rows
])
con.commit()
t=time.perf_counter()
page=con.execute("SELECT asset_id FROM asset_search WHERE asset_search MATCH ? LIMIT 240 OFFSET 4800",('"blue"*',)).fetchall()
fts_elapsed=time.perf_counter()-t
assert len(page)==240
t=time.perf_counter()
cjk_page=con.execute("""
    SELECT a.id FROM assets a
    JOIN asset_cjk_search ON asset_cjk_search.asset_id=a.id
    LEFT JOIN prompt_state ps ON ps.asset_id=a.id
    WHERE asset_cjk_search MATCH ?
      AND (a.name LIKE ? OR COALESCE(ps.prompt,'') LIKE ?)
    LIMIT 240
""",('"蓝发角色"','%蓝发角色%','%蓝发角色%')).fetchall()
cjk_elapsed=time.perf_counter()-t
assert len(cjk_page)==240
assert cjk_elapsed < 1.0
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
query=[1/math.sqrt(16)]*16
t=time.perf_counter()
semantic_rows=con.execute("SELECT asset_id,vector FROM semantic_embeddings WHERE model_id=? AND dimensions=?",('clip-vit-b32-qdrant-v1',16)).fetchall()
scores=[]
for aid,blob in semantic_rows:
    vector=struct.unpack('<16f',blob)
    scores.append((sum(a*b for a,b in zip(query,vector)),aid))
top=sorted(scores,reverse=True)[:240]
semantic_elapsed=time.perf_counter()-t
assert len(semantic_rows)==N and len(top)==240
assert semantic_elapsed < 3.0
con.close()
print(f'ImageLore 50k search smoke test: PASS (FTS {fts_elapsed:.4f}s, CJK-trigram {cjk_elapsed:.4f}s, generation {advanced_elapsed:.4f}s, semantic-linear {semantic_elapsed:.4f}s)')
