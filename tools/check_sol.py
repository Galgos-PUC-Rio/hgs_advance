"""Independent VRPTW solution checker: d(i,j) = nint(euclid), travel time = d."""
import math, sys
def load(inst):
    L = open(inst).read().split('\n')
    nv, cap = map(int, L[4].split()[:2])
    nodes = []
    for l in L[9:]:
        p = l.split()
        if len(p) >= 7: nodes.append(tuple(int(x) for x in p[:7]))
    return cap, nodes
def check(inst, sol):
    cap, N = load(inst)
    d = lambda a, b: int(math.floor(math.hypot(N[a][1]-N[b][1], N[a][2]-N[b][2]) + 0.5))
    routes = [list(map(int, l.split(':')[1].split())) for l in open(sol) if l.startswith('Route')]
    seen, total = [], 0
    for r in routes:
        t, load_, prev = 0, 0, 0
        for c in r + [0]:
            t += d(prev, c); total += d(prev, c)
            assert t <= N[c][5], f"late at {c}: {t} > {N[c][5]}"
            t = max(t, N[c][4]) + N[c][6]
            load_ += N[c][3]; prev = c
        assert load_ <= cap, f"capacity {load_} > {cap}"
        seen += r
    assert sorted(seen) == list(range(1, len(N))), "not every customer exactly once"
    return len(routes), total
if __name__ == '__main__':
    print("FEASIBLE: routes=%d distance=%d" % check(sys.argv[1], sys.argv[2]))
