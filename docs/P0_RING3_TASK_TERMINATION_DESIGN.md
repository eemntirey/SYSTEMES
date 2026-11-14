# Conception — terminaison d'une tâche Ring 3 (niveau 2, hors périmètre P0 actuel)

**Statut :** conception uniquement. Aucun code d'ordonnanceur n'est modifié par la phase P0 de récupération synchrone.

## Interface manquante

1. **Propriété du contexte :** une tâche doit posséder un objet de contexte utilisateur contenant au minimum son espace d'adressage (CR3/AddressSpace), RIP, RSP, registres utilisateur, statut de sortie et état de nettoyage. Le contexte doit être lié à un identifiant de tâche, et non au seul SyscallCpuLocal du CPU.
2. **Point de terminaison :** ajouter une opération noyau du type terminate_current_user_context(reason), appelée depuis le chemin d'exception après validation du CS sauvegardé. Elle marque la tâche courante Exited, enregistre le vecteur/code d'erreur/RIP/CR2, et interdit toute reprise de son contexte Ring 3.
3. **Sélection suivante :** la terminaison doit passer à un chemin de non-retour sur pile noyau, puis invoquer le sélecteur existant de la file locale. Elle ne doit jamais retourner vers le contexte marqué Exited.
4. **Nettoyage :** le retrait des mappings et la libération des frames ne se font qu'après preuve que le CR3 du contexte n'est plus actif sur aucun CPU. En SMP, prévoir un état Reaping et un rendez-vous/TLB shootdown avant la libération, avec propriétaire unique pour éviter double libération.
5. **État CPU :** la continuation de terminaison conserve GS noyau, RSP0/TSS et drapeaux cohérents. Le retour utilisateur n'est possible qu'après installation complète du contexte de la tâche suivante.

## Impact ciblé sur kernel/src/scheduler.rs

- **Autour de schedule / sélection de tâche (environ lignes 470–590) :** ajouter une transition explicite Running -> Exited et exclure la tâche terminée de la sélection. Le chemin courant schedule(saved_rsp, ...) préserve parfois le contexte courant ; il faut un mode de sortie non-retournant distinct.
- **Autour de on_yield et on_timer (environ lignes 590–625) :** exposer une entrée étroite schedule_after_exit(saved_rsp) ou un paramètre de raison de commutation qui garantit que la tâche courante ne redevient pas Runnable.
- **Autour de reset_after_all_tasks_exit (environ lignes 700+) :** ne pas réutiliser cette fonction comme mécanisme de terminaison d'un processus ; elle réinitialise l'ordonnanceur seulement lorsque le CPU est revenu sur le contexte boot/idle.
- **À ajouter hors du scheduler :** une file de réclamation différée des espaces d'adressage, pilotée après rendez-vous SMP/TLB, afin que le chemin d'exception ne libère jamais la pile ni les tables encore actives.

## Invariants requis avant implémentation

- Une seule tâche est propriétaire de chaque contexte et de chaque espace d'adressage.
- Une tâche Exited ne peut plus être sélectionnée ni migrée.
- Le CPU fautif a quitté le CR3 utilisateur avant tout nettoyage.
- Tous les autres CPU ont acquitté le détachement de l'espace avant la libération.
- Un seul reaper libère les frames ; les répétitions de terminaison sont idempotentes.

**Décision de périmètre :** le niveau 2 n'est pas implémenté dans la PR P0 de récupération synchrone Ring 3.