from pathlib import Path
import sqlite3,tempfile,time
root=Path(__file__).resolve().parents[1]
schema=(root/'src-tauri/schema.sql').read_text()
N=10000
with tempfile.TemporaryDirectory() as td:
    con=sqlite3.connect(Path(td)/'library.sqlite3')
    con.executescript(schema)
    now=1760000000
    con.executemany("INSERT INTO assets(path,name,created_at,updated_at) VALUES(?,?,?,?)",[(f'/tmp/{i}.png',f'asset {i}',now+i%100,now+i%100) for i in range(N)])
    rows=con.execute('select id,name from assets').fetchall()
    con.executemany("INSERT INTO prompt_state(asset_id,prompt,negative_prompt,model,updated_at) VALUES(?,?,?,?,?)",[(i,f'prompt blue character {i}','','GPT Image' if i%2 else 'Flux',now) for i,_ in rows])
    con.executemany("INSERT INTO asset_search(asset_id,name,prompt,negative_prompt,model,tags) VALUES(?,?,?,?,?,?)",[(i,name,f'prompt blue character {i}','','GPT Image' if i%2 else 'Flux','') for i,name in rows])
    con.commit()
    t=time.perf_counter(); page=con.execute("SELECT asset_id FROM asset_search WHERE asset_search MATCH ? LIMIT 240 OFFSET 4800",('"blue"*',)).fetchall(); elapsed=time.perf_counter()-t
    assert len(page)==240
    con.close()
print(f'ImageLore 10k FTS/page smoke test: PASS ({elapsed:.4f}s)')
