// Jam locale (#25) : contrôles de l'hôte dans Réglages › Application.

export const fr = {
  title: "Jam",
  hint: "Des invités sur le même réseau ajoutent des titres à votre file depuis leur téléphone, sans app ni compte. Seul cet ordinateur joue la musique.",
  start: "Démarrer la Jam",
  stop: "Arrêter",
  starting: "Démarrage…",
  address: "Adresse",
  code: "Code",
  copy: "Copier",
  copied: "Adresse copiée",
  qr: "QR code de la Jam",
  howTo: "Les invités scannent le QR code, ou ouvrent l'adresse dans leur navigateur et saisissent le code. Leur prénom s'affiche à côté de leurs titres dans la file. Vous gardez la main sur la file : retirez un titre depuis la file habituelle.",
  firewall: "Au premier démarrage, Windows demande d'autoriser Seaglass sur le réseau : acceptez pour les réseaux privés.",
  noLan: "Aucun réseau local trouvé : seuls les navigateurs de cet ordinateur peuvent rejoindre la Jam.",
  stopped: "Jam arrêtée",
  addedBy: (name: string) => `ajouté par ${name}`,
};

export const en: typeof fr = {
  title: "Jam",
  hint: "Guests on the same network add tracks to your queue from their phone, with no app or account. Only this computer plays the music.",
  start: "Start the Jam",
  stop: "Stop",
  starting: "Starting…",
  address: "Address",
  code: "Code",
  copy: "Copy",
  copied: "Address copied",
  qr: "Jam QR code",
  howTo: "Guests scan the QR code, or open the address in their browser and enter the code. Their first name appears next to their tracks in the queue. You stay in control: remove a track from the usual queue.",
  firewall: "The first time, Windows asks to allow Seaglass on the network: accept for private networks.",
  noLan: "No local network found: only browsers on this computer can join the Jam.",
  stopped: "Jam stopped",
  addedBy: (name: string) => `added by ${name}`,
};
