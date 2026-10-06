import re, sys, os
sys.path.insert(0, os.path.dirname(__file__))
from check_sol import check
inst, out, cfg, seeds = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4].split()
stem = os.path.basename(inst).rsplit('.', 1)[0]
names = [l.split()[0] for l in open(cfg) if l.strip() and not l.strip().startswith('#')]
print("%-14s %s | %s" % ("config", " ".join("%-17s" % ("seed " + s) for s in seeds), "best / mean dist / max routes"))
for n in names:
    cells, res = [], []
    for s in seeds:
        log = open(f"{out}/{n}_s{s}.log").read()
        m = re.search(r"(\d+) routes, total distance = (\d+) \(best found at ([\d.]+)s\)", log)
        sol = f"{out}/{n}_s{s}/{stem}.sol"
        try:
            r, d = check(inst, sol); ok = m and (int(m.group(1)), int(m.group(2))) == (r, d)
            cells.append("%2dr %6d @%5.1fs%s" % (r, d, float(m.group(3)), "" if ok else "!")); res.append((r, d))
        except Exception as e:
            cells.append("%-17s" % "FAIL"); 
    if res:
        mr = max(r for r, _ in res); ds = [d for r, d in res]
        b = min(res); summ = "%dr %d / %.0f / %d" % (b[0], b[1], sum(ds) / len(ds), mr)
    else: summ = "-"
    print("%-14s %s | %s" % (n, " ".join(cells), summ))
