# Changelog

Les changements notables de SYSTEMES sont consignés ici. Les dates suivent UTC.

## [Unreleased]

### Documentation
- Ajout d'une roadmap fonctionnelle de 26 domaines avec états vérifiés et chemins de code.
- Formalisation de l'architecture à deux produits : édition Windows hôte et noyau autonome Rust.
- Documentation des transports WebView2 et JSON stdin/stdout, des frontières de sécurité et des limites actuelles du noyau.
- Clarification des validations CI disponibles et des validations encore non établies.

## 2026-11-05 — État de référence audité

- HEAD `main` : `0b732fd64e139ce45d59eca8f09d304b82179f10` — K3.1.2 cycle de vie natif.
- CI principale sur ce HEAD : réussie (run 41993456370).
- Le noyau Rust possède le socle UEFI, GDT/IDT, exceptions, allocateur physique et pagination de base ; le shell autonome et les primitives OS avancées restent à développer.
