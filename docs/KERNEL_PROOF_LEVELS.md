# Niveaux de preuve du noyau SYSTEMES

Les niveaux sont cumulatifs : une capacité n'est déclarée QEMU-validée que si les tests hôtes, la compilation cible et les marqueurs QEMU/OVMF sont tous réussis sur le SHA exact. Le matériel physique reste non validé sans rapport de démarrage réel.

| Capacité | Implémentée | Compilée x86_64 | Tests hôtes | QEMU/OVMF | Matériel physique |
|---|---:|---:|---:|---:|---:|
| Heap noyau 1 Mio — PR #49, HEAD `06bbd9e75c4d3febd43f4b43f9462aae499c26e1`, merge `6e6bb8095a6d7c624a0f15f31f50d3e06bba7de9` | Oui | Oui | 28/28 | Oui | Non validé |
| Découverte ACPI/MADT — PR #50, HEAD `fac7389b750884a85bfb28cdee943df2816cca4a`, merge `499713321df57ad329b526720f39eba3a6ca3102` | Oui | Oui | 33/33 | Oui | Non validé |
| Timer Local APIC et horloge monotone — PR #51, HEAD `a5a04096598939f276ddeb09d6ccd4296623d55d`, merge `df7ff3632de342e0331acb8975bf9da941fedcd5` | Oui | Oui | 37/37 | Oui | Non validé |
| Ordonnanceur coopératif — PR #52, HEAD `c1a8b98343ae2242e916011f64c618fdce01a322`, merge `3a4ed1e10d37dd817aed67828490339e23e82208` | Oui | Oui | 40/40 | Oui | Non validé |
| Ordonnanceur préemptif — PR #53, merge `a32bb337dfe52793a424d960433b8df84d3a2dc8` | Oui | Oui | 40/40 | Oui | Non validé |
| Contexte x87/SSE par tâche — PR #54 intégré | FXSAVE64/FXRSTOR64 | Oui | 41/41 | Oui | Non validé |
| TSS/RSP0/IST — PR #56, HEAD `e6b9e8b6981506e770a43964bdf0aac56a6afb25`, run `42374846321` | Oui | Oui | Oui | Oui (6/6 checks) | Non validé |
| Blocage/sommeil/réveil de tâches — PR #57 en cours | Implémentation en cours | En attente CI | En attente CI | En attente QEMU | Non validé |

## Limites actuellement confirmées

- Heap : arène statique de 1 Mio ; pas encore d'extension par pages.
- ACPI : MADT xAPIC/IOAPIC/overrides ; enregistrements x2APIC non consommés.
- Timer : xAPIC mono-cœur, 100 Hz ; pas de SMP.
- Ordonnanceur coopératif : tâches noyau partageant le même espace d'adressage ; yield explicite.
- Ordonnanceur préemptif : trois tâches CPU-bound validées sous préemption timer QEMU ; mono-cœur uniquement.
- FPU/SIMD : x87/MMX/SSE sauvegardés/restaurés par tâche ; tests x87 et XMM15 validés sous QEMU.
- TSS/IST : TR=0x18, RSP0 et routage IST1/2/3 vérifiés ; IST2/IST3 testés par interruptions logicielles, pas par NMI/Machine Check électriques ni Double Fault réel.
- Aucun de ces jalons n'est validé sur matériel physique.
