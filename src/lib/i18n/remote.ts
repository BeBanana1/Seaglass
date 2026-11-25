// Remote (#336) and playback handoff (#414, #62): Settings › App.

export const fr = {
  title: "Télécommande",
  hint: "Pilotez la lecture de cet ordinateur depuis le navigateur de votre téléphone : pause, titres suivants, volume, file, albums et playlists. Sur le même Wi-Fi seulement.",
  enable: "Télécommande",
  enableHint: "Reste allumée d'un lancement à l'autre, avec le même code.",
  qr: "QR code de la télécommande",
  address: "Adresse",
  code: "Code",
  copy: "Copier l'adresse",
  copied: "Adresse copiée",
  renew: "Nouveau code",
  renewed: "Nouveau code : scannez de nouveau le QR code sur vos téléphones",
  howTo:
    "Scannez le QR code une fois avec votre téléphone : il garde le code. Ce code donne le contrôle de la lecture, ne le partagez pas ; « Nouveau code » déconnecte les téléphones déjà appairés.",
  noLan:
    "Aucun réseau local trouvé : seuls les navigateurs de cet ordinateur peuvent ouvrir la télécommande.",
  firewall:
    "Au premier allumage, Windows demande d'autoriser Seaglass : acceptez pour les réseaux privés.",
  handoff: "Accepter les transferts",
  handoffHint:
    "L'app Android liée peut reprendre ici ce qui joue sur le téléphone, ou continuer sur le téléphone ce qui joue ici, et les deux échangent leur historique d'écoute.",
  handoffHowToLan:
    "Sur le téléphone : Réglages › Application › Ordinateur lié, puis tapez l'adresse et le code ci-dessus.",
  handoffHowTo:
    "Appuyez sur « Lier un téléphone », puis sur le téléphone : Réglages › Application › Ordinateur lié, et tapez l'adresse et le code ci-dessus dans les 10 minutes. Le téléphone apprend alors aussi à joindre cet ordinateur hors de chez vous.",
  pair: "Lier un téléphone",
  pairing: (until: string) => `Un téléphone peut se lier jusqu'à ${until}.`,
  pairNote:
    "Chaque « Lier un téléphone » change la clé du relais : les téléphones liés avant gardent le Wi-Fi, mais doivent être liés de nouveau pour vous joindre hors de chez vous. Liez vos appareils dans la même fenêtre.",
};

export const en: typeof fr = {
  title: "Remote",
  hint: "Control this computer's playback from your phone's browser: pause, skip, volume, queue, albums and playlists. Same Wi-Fi only.",
  enable: "Remote",
  enableHint: "Stays on from one launch to the next, with the same code.",
  qr: "Remote QR code",
  address: "Address",
  code: "Code",
  copy: "Copy address",
  copied: "Address copied",
  renew: "New code",
  renewed: "New code: scan the QR code again on your phones",
  howTo:
    "Scan the QR code once with your phone: it keeps the code. This code gives control of playback, do not share it; “New code” disconnects the phones already paired.",
  noLan: "No local network found: only browsers on this computer can open the remote.",
  firewall:
    "The first time it is turned on, Windows asks to allow Seaglass: accept for private networks.",
  handoff: "Accept transfers",
  handoffHint:
    "The linked Android app can resume here what plays on the phone, or continue on the phone what plays here, and both exchange their listening history.",
  handoffHowToLan:
    "On the phone: Settings › App › Linked computer, then type the address and code above.",
  handoffHowTo:
    "Press “Link a phone”, then on the phone: Settings › App › Linked computer, and type the address and code above within 10 minutes. The phone then also learns how to reach this computer away from home.",
  pair: "Link a phone",
  pairing: (until: string) => `A phone can link until ${until}.`,
  pairNote:
    "Each “Link a phone” changes the relay key: phones linked before keep the Wi-Fi, but must be linked again to reach you away from home. Link your devices in the same window.",
};
