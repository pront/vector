Docker images no longer trust the `emSign Root CA - C1`, `emSign ECC Root CA - C3`, `emSign ECC Root CA - G3`, and `AAA Certificate Services` roots. Removing AAA also blocks the alternate trust path through Sectigo cross-signed emSign intermediates (CVE-2026-111943).

authors: andyqvo
