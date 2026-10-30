#!/usr/bin/env python3
"""Draws docs/img/phases.svg and its Spanish twin docs/es/img/phases.svg: the phases and gates
of GDD with the way back when a gate says no, for explanation/method.md.

    python3 scripts/make_phases_svg.py

Mermaid tangles a diagram with this many loops, so this one is drawn by hand. Change the words
in EN and ES below, run the script, and look at both images before you commit them.
"""

from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def svg(t: dict) -> str:
    W_,H_=600,930
    cx=280; bw=180; gw=210; bh=44; gh=40
    ys={k: v + 50 for k, v in {"S":96,"G0":160,"D":224,"G1":288,"B":352,"G2":416,"I":480,"G3":544,"R":608,"G4":672}.items()}
    BG="#0d1020";PANEL="#1a1f3d";LINE="#2b3263";TX="#e8eaf6";MUT="#a3a9cc";HUM="#d6a6ff";MAC="#4fd18b";RED="#ff6b6b";AMB="#ffb36b"
    o=[f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W_} {H_}" role="img" aria-label="{t["aria"]}" font-family="Inter, system-ui, sans-serif">',
       f'<rect width="100%" height="100%" rx="16" fill="{BG}"/>',
       '<defs>',
       f'<marker id="a" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z" fill="{MUT}"/></marker>',
       f'<marker id="r" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z" fill="{RED}"/></marker>',
       f'<marker id="m" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z" fill="{AMB}"/></marker>',
       '</defs>',
       f'<text x="24" y="36" fill="{TX}" font-size="19" font-weight="700">{t["title"]}</text>',
       f'<rect x="20" y="52" width="{W_-40}" height="62" rx="10" fill="{PANEL}" stroke="{LINE}" stroke-width="1"/>',
       f'<rect x="36" y="64" width="14" height="14" rx="3" fill="#2a2350" stroke="{HUM}" stroke-width="2"/><text x="58" y="76" fill="{MUT}" font-size="13">{t["human"]}</text>',
       f'<rect x="{36+t["lg"]}" y="64" width="14" height="14" rx="3" fill="{PANEL}" stroke="{MAC}" stroke-width="2"/><text x="{58+t["lg"]}" y="76" fill="{MUT}" font-size="13">{t["machines"]}</text>',
       f'<line x1="36" y1="98" x2="66" y2="98" stroke="{RED}" stroke-width="2" stroke-dasharray="6 4" marker-end="url(#r)"/><text x="76" y="102" fill="{MUT}" font-size="13">{t["ko"]}</text>',
       f'<line x1="{36+t["lg"]}" y1="98" x2="{66+t["lg"]}" y2="98" stroke="{AMB}" stroke-width="2" stroke-dasharray="3 5" marker-end="url(#m)"/><text x="{76+t["lg"]}" y="102" fill="{MUT}" font-size="13">{t["learnleg"]}</text>']
    def box(k,label,sub=None):
        y=ys[k]; o.append(f'<rect x="{cx-bw/2}" y="{y-bh/2}" width="{bw}" height="{bh}" rx="10" fill="{PANEL}" stroke="{LINE}" stroke-width="1.5"/>')
        if sub:
            o.append(f'<text x="{cx}" y="{y-3}" text-anchor="middle" fill="{TX}" font-size="15" font-weight="600">{label}</text><text x="{cx}" y="{y+14}" text-anchor="middle" fill="{MUT}" font-size="11.5">{sub}</text>')
        else: o.append(f'<text x="{cx}" y="{y+5}" text-anchor="middle" fill="{TX}" font-size="15" font-weight="600">{label}</text>')
    def gate(k,label,human):
        y=ys[k]; col=HUM if human else MAC; fill="#2a2350" if human else PANEL
        x0,x1=cx-gw/2,cx+gw/2; h=gh/2
        o.append(f'<path d="M{x0+14} {y-h} H{x1-14} L{x1} {y} L{x1-14} {y+h} H{x0+14} L{x0} {y} Z" fill="{fill}" stroke="{col}" stroke-width="2"/>')
        o.append(f'<text x="{cx}" y="{y+5}" text-anchor="middle" fill="{col}" font-size="14" font-weight="700">{label}</text>')
    order=["S","G0","D","G1","B","G2","I","G3","R","G4"]
    for a_,b_ in zip(order,order[1:]):
        ya=ys[a_]+(gh/2 if a_.startswith("G") else bh/2); yb=ys[b_]-(gh/2 if b_.startswith("G") else bh/2)
        o.append(f'<line x1="{cx}" y1="{ya}" x2="{cx}" y2="{yb-2}" stroke="{MUT}" stroke-width="2" marker-end="url(#a)"/>')
    box("S",t["setup"],t["once"]); gate("G0","G0 · "+t["g0"],False); box("D","1 Discover"); gate("G1","G1 · "+t["g1"],True)
    box("B","2 Build"); gate("G2","G2 · "+t["g2"],False); box("I","3 Integrate"); gate("G3","G3 · "+t["g3"],False); box("R","4 Release"); gate("G4","G4 · "+t["g4"],True)
    def pill(x,y,text,col):
        w=len(text)*6.6+18
        o.append(f'<rect x="{x-w/2}" y="{y-11}" width="{w}" height="22" rx="11" fill="{BG}" stroke="{col}" stroke-width="1.2"/><text x="{x}" y="{y+4.5}" text-anchor="middle" fill="{col}" font-size="12">{text}</text>')
    def back(src,dst,x,dy_end,label,ly):
        y1=ys[src]; y2=ys[dst]+dy_end; xs=cx+gw/2; xe=cx+bw/2
        o.append(f'<path d="M{xs} {y1} H{x-12} Q{x} {y1} {x} {y1-12} V{y2+12} Q{x} {y2} {x-12} {y2} H{xe+3}" fill="none" stroke="{RED}" stroke-width="2" stroke-dasharray="6 4" marker-end="url(#r)"/>')
    back("G0", "S", 420, 0, "", 0)
    back("G1", "D", 420, 0, "", 0)
    back("G2", "B", 420, -10, "", 0)
    back("G3", "B", 450, 2, "", 0)
    back("G4", "B", 480, 12, "", 0)
    xl=70; y1=ys["G4"]; y2=ys["D"]
    o.append(f'<path d="M{cx-gw/2} {y1} H{xl+12} Q{xl} {y1} {xl} {y1-12} V{y2+12} Q{xl} {y2} {xl+12} {y2} H{cx-bw/2-3}" fill="none" stroke="{MUT}" stroke-width="2" marker-end="url(#a)"/>')
    pill(xl,498,t["next"],TX)
    lx,ly0,lw,lh=305,792,275,104
    o.append(f'<path d="M{cx} {ys["G4"]+gh/2} V{ly0+lh/2} H{lx-3}" fill="none" stroke="{AMB}" stroke-width="2" stroke-dasharray="3 5" marker-end="url(#m)"/>')
    pill(cx,780,t["sometimes"],AMB)
    o.append(f'<rect x="{lx}" y="{ly0}" width="{lw}" height="{lh}" rx="12" fill="{PANEL}" stroke="{AMB}" stroke-width="1.5" stroke-dasharray="5 4"/>')
    o.append(f'<text x="{lx+16}" y="{ly0+30}" fill="{AMB}" font-size="15" font-weight="700">5 Learn</text>')
    for i,line in enumerate(t["learn"]):
        o.append(f'<text x="{lx+16}" y="{ly0+54+i*19}" fill="{TX}" font-size="12.5">{line}</text>')
    o.append('</svg>')
    return "\n".join(o)+"\n"


EN = {'aria': 'The six phases and five gates of GDD, with the way back when a gate says no', 'title': 'Phases and gates', 'human': 'a person decides', 'machines': 'the machines decide', 'lg': 270, 'setup': '0 Setup', 'once': 'once per project', 'g0': 'Setup Gate', 'g1': 'Scenario Gate', 'g2': 'Loop Gates', 'g3': 'Merge Gate', 'g4': 'Release Gate', 'next': 'next change', 'sometimes': 'from time to time', 'learn': ['Repeated problems become rules,', 'lints and tests that G2 and G3', 'check on every later change.'], 'ko': 'a gate says no: back to fix it', 'learnleg': 'Learn hardens the gates'}

ES = {'aria': 'Las seis fases y las cinco puertas de GDD, con la vuelta atrás cuando una puerta dice que no', 'title': 'Fases y puertas', 'human': 'decide una persona', 'machines': 'deciden las máquinas', 'lg': 290, 'setup': '0 Setup', 'once': 'una vez por proyecto', 'g0': 'Preparación', 'g1': 'Escenarios', 'g2': 'Bucle', 'g3': 'Integración', 'g4': 'Salida', 'next': 'siguiente Change', 'sometimes': 'de vez en cuando', 'learn': ['Lo que se repite se vuelve reglas,', 'lints y tests que G2 y G3 comprueban', 'en todos los Changes siguientes.'], 'ko': 'una puerta dice que no: vuelve atrás', 'learnleg': 'Learn refuerza las puertas'}


if __name__ == "__main__":
    for lang, words in (("img", EN), ("es/img", ES)):
        path = ROOT / "docs" / lang / "phases.svg"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(svg(words), encoding="utf-8")
        print(f"wrote {path.relative_to(ROOT)}")
