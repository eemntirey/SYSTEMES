# K2.9 — Calibration du Local APIC et horloge monotone

## Objectif

Remplacer le compte initial provisoire par une mesure effectuée au démarrage,
puis exposer une horloge monotone du noyau. Le timer d'ordonnancement et
l'horloge utilisent le même événement matériel Local APIC.

## Méthode de calibration

- Le PIT conserve sa référence architecturale de 1 193 182 Hz.
- Le canal 2 du PIT est programmé en mode 0 avec un décompte de 59 659,
  soit environ 50 ms.
- Le Local APIC est configuré avec le diviseur 16 et un compteur one-shot
  initialisé à 0xFFFFFFFF, livraison masquée pendant la mesure.
- Le noyau attend la fin du décompte PIT via OUT2 (bit 5 du port 0x61), puis
  lit le Current Count Register du Local APIC (offset 0x390).
- La fréquence du compteur divisé est calculée à partir du nombre de décréments
  observés et de la durée PIT. Le compte périodique à 100 Hz est dérivé de
  cette mesure ; aucune constante APIC arbitraire n'est utilisée.
- Les erreurs de timeout, les mesures nulles et les comptes non représentables
  font échouer l'initialisation au lieu de démarrer avec une fréquence supposée.

La calibration restaure l'état du port de contrôle du haut-parleur et masque
le timer APIC one-shot avant de configurer le mode périodique.

## Horloge monotone

`kernel/src/clock.rs` expose :

- `clock::ticks()` : nombre de ticks depuis l'initialisation ;
- `clock::now_millis()` : temps monotone écoulé en millisecondes ;
- `clock::now_nanos()` : temps monotone écoulé en nanosecondes.

La résolution initiale est de 10 ms à 100 Hz. L'horloge ne représente pas
l'heure civile UTC et ne dépend pas des réglages de date/heure. Elle avance
uniquement sur réception d'une interruption du timer Local APIC.

## Validation automatisée

Les tests hôtes vérifient le calcul de fréquence, le calcul du compte 100 Hz,
le rejet des entrées nulles et des résultats non représentables, ainsi que
l'avancement monotone de l'horloge.

Les scénarios QEMU scheduler et FPU doivent également retrouver les marqueurs :

- `SYSTEMES: Local APIC timer calibrated against PIT channel 2 at 100 Hz`
- `SYSTEMES: monotonic kernel clock initialized at 100 Hz`
- `SYSTEMES: Local APIC periodic scheduler timer configured`

La CI doit être verte sur le HEAD exact de la PR avant toute fusion. Les tests
QEMU ne remplacent pas encore une validation sur matériel physique.

## Limites

- La calibration utilise le PIT channel 2 ; les plateformes sans PIT compatible
  devront fournir ultérieurement une source de référence alternative (HPET ou
  TSC invariant calibré).
- L'horloge est à résolution 10 ms et ne fournit pas encore de timers one-shot,
  de sleep/wakeup temporisé ni de calendrier.
- La mesure est mono-CPU. Le SMP exigera une calibration et un état timer par CPU,
  avec une source monotone cohérente entre processeurs.
