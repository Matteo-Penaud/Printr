//! Énigme du jour : devinettes, charades, logique et calcul, en local et hors ligne.
//! Chaque jour tire une énigme différente ; toute la liste passe avant la première répétition.

use anyhow::{Result, bail};
use chrono::NaiveDate;
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use serde::Deserialize;

use crate::doc::{Align, Doc, Style};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Devinette,
    Charade,
    Logique,
    Calcul,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::Devinette => crate::i18n::t("devinette", "classic"),
            Kind::Charade => "charade",
            Kind::Logique => crate::i18n::t("logique", "logic"),
            Kind::Calcul => crate::i18n::t("calcul", "math"),
        }
    }
}

/// Où imprimer la réponse.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Answer {
    /// À l'envers en bas du ticket : on le retourne pour lire.
    #[default]
    #[serde(alias = "envers")]
    UpsideDown,
    /// Réponse de l'énigme de la veille.
    #[serde(alias = "lendemain")]
    NextDay,
    /// À l'endroit, sous l'énigme.
    #[serde(alias = "dessous")]
    Below,
    #[serde(alias = "aucune")]
    None,
}

use Kind::*;

/// (famille, énigme, réponse)
#[rustfmt::skip]
const RIDDLES: &[(Kind, &str, &str)] = &[
    // Devinettes
    (Devinette, "Plus on m'enlève, plus je grandis. Qui suis-je ?", "Un trou."),
    (Devinette, "On peut me tenir ou me rompre sans jamais me toucher. Qui suis-je ?", "Une promesse."),
    (Devinette, "Je monte chaque année et je ne redescends jamais. Qui suis-je ?", "L'âge."),
    (Devinette, "J'ai des dents mais je ne mords jamais. Qui suis-je ?", "Un peigne."),
    (Devinette, "J'ai un cou mais pas de tête. Qui suis-je ?", "Une bouteille."),
    (Devinette, "J'ai des aiguilles mais je ne couds pas. Qui suis-je ?", "Une horloge (ou un sapin !)."),
    (Devinette, "Je cours sans jamais marcher, j'ai un lit mais je ne dors jamais. Qui suis-je ?", "Une rivière."),
    (Devinette, "Je suis pleine de trous et pourtant je retiens l'eau. Qui suis-je ?", "Une éponge."),
    (Devinette, "Plus je sèche, plus je suis mouillée. Qui suis-je ?", "Une serviette."),
    (Devinette, "Il t'appartient, mais les autres s'en servent bien plus que toi. Qu'est-ce que c'est ?", "Ton prénom."),
    (Devinette, "Je parle sans bouche et j'entends sans oreilles. Qui suis-je ?", "L'écho."),
    (Devinette, "Il suffit de prononcer mon nom pour me briser. Qui suis-je ?", "Le silence."),
    (Devinette, "J'ai des villes sans maisons, des forêts sans arbres et des rivières sans eau. Qui suis-je ?", "Une carte."),
    (Devinette, "Je monte et je descends sans jamais bouger. Qui suis-je ?", "Un escalier."),
    (Devinette, "J'ai des feuilles mais je ne suis pas un arbre. Qui suis-je ?", "Un livre."),
    (Devinette, "J'ai une tête et une queue, mais pas de corps. Qui suis-je ?", "Une pièce de monnaie."),
    (Devinette, "Plus il y en a, moins on y voit. Qu'est-ce que c'est ?", "L'obscurité."),
    (Devinette, "Je fais le tour du monde sans quitter mon coin. Qui suis-je ?", "Un timbre."),
    (Devinette, "J'ai des touches mais je n'ouvre aucune porte. Qui suis-je ?", "Un piano (ou un clavier)."),
    (Devinette, "J'ai un œil mais je ne vois rien. Qui suis-je ?", "Une aiguille."),
    (Devinette, "Je remplis une pièce sans y prendre la moindre place. Qui suis-je ?", "La lumière."),
    (Devinette, "Je suis toujours devant toi, mais tu ne peux jamais me voir. Qui suis-je ?", "L'avenir."),
    (Devinette, "Je grandis quand on me met la tête en bas. Qui suis-je ?", "Le chiffre 6, qui devient 9."),
    (Devinette, "Je suis noir quand je suis propre et blanc quand je suis sale. Qui suis-je ?", "Un tableau noir."),
    (Devinette, "Qui marche à quatre pattes le matin, sur deux à midi et sur trois le soir ?", "L'être humain : bébé à quatre pattes, adulte debout, vieillard avec une canne (l'énigme du Sphinx)."),
    (Devinette, "Je commence la nuit et je termine le matin. Qui suis-je ?", "La lettre N."),
    (Devinette, "Qu'est-ce qui se trouve au milieu de Paris ?", "La lettre R."),
    (Devinette, "On me trouve une fois dans une minute, deux fois dans un moment, et jamais en cent ans. Qui suis-je ?", "La lettre M."),
    (Devinette, "J'ai des bras mais pas de mains. Qui suis-je ?", "Un fauteuil."),
    (Devinette, "J'ai un pied mais pas de jambe. Qui suis-je ?", "Un champignon (ou une lampe)."),
    (Devinette, "Je tombe sans jamais me faire mal. Qui suis-je ?", "La pluie (ou la nuit)."),
    (Devinette, "On peut m'attraper, mais jamais me lancer. Qui suis-je ?", "Un rhume."),
    (Devinette, "Je suis léger comme une plume, mais personne ne peut me retenir longtemps. Qui suis-je ?", "Le souffle."),
    (Devinette, "Plus je suis chaud, plus je suis frais. Qui suis-je ?", "Le pain."),
    (Devinette, "Je fais le tour de la maison sans jamais bouger. Qui suis-je ?", "Le mur (ou la clôture)."),
    (Devinette, "Je passe devant le soleil sans jamais faire d'ombre. Qui suis-je ?", "Le vent."),
    (Devinette, "On me jette quand on a besoin de moi et on me remonte quand on n'en a plus besoin. Qui suis-je ?", "Une ancre."),
    (Devinette, "Nous sommes pleines le jour et vides la nuit. Qui sommes-nous ?", "Les chaussures."),
    (Devinette, "Je suis noir quand on m'achète, rouge quand on m'utilise et gris quand on me jette. Qui suis-je ?", "Le charbon."),
    (Devinette, "J'ai des yeux mais je ne vois rien, et je pousse sous la terre. Qui suis-je ?", "La pomme de terre."),
    (Devinette, "Je me lève le matin et je me couche le soir sans jamais dormir. Qui suis-je ?", "Le soleil."),
    (Devinette, "Je suis plus grand que Dieu, pire que le diable ; les pauvres m'ont, les riches ont besoin de moi, et si on me mange, on meurt. Qui suis-je ?", "Rien."),
    (Devinette, "J'ai treize cœurs mais aucun organe. Qui suis-je ?", "Un jeu de cartes."),
    (Devinette, "Je mange tout ce que je touche, mais un peu d'eau suffit à me tuer. Qui suis-je ?", "Le feu."),
    (Devinette, "On me coupe, on me sert, mais on ne me mange jamais. Qui suis-je ?", "Un jeu de cartes."),
    (Devinette, "Plus je travaille, plus je raccourcis. Qui suis-je ?", "Une bougie (ou un crayon)."),
    (Devinette, "Je suis pris avant d'être développé. Qui suis-je ?", "Une photo argentique."),
    (Devinette, "J'ai des racines mais je ne suis pas une plante, et on me soigne chez le dentiste. Qui suis-je ?", "Une dent."),
    (Devinette, "Je vole sans ailes et je pleure sans yeux. Qui suis-je ?", "Un nuage."),
    (Devinette, "Quel bateau se lit aussi bien de gauche à droite que de droite à gauche ?", "Le kayak."),
    (Devinette, "Quel mot de six lettres contient les cinq voyelles a, e, i, o et u ?", "Oiseau."),
    (Devinette, "Qu'est-ce qui est jaune et qui attend ?", "Jonathan."),
    (Devinette, "Quel est l'animal le plus heureux ?", "Le hibou, parce que sa femme est chouette."),
    (Devinette, "Je suis le seul endroit où aujourd'hui vient avant hier. Qui suis-je ?", "Le dictionnaire."),
    (Devinette, "Je m'allonge quand on me coupe des deux côtés. Qui suis-je ?", "Un fossé."),
    // Charades
    (Charade, "Mon premier miaule. Mon second recouvre le corps. Mon tout se porte sur la tête.", "Chapeau (chat + peau)."),
    (Charade, "Mon premier est le contraire de haut. Mon second est le contraire de tard. Mon tout flotte sur l'eau.", "Bateau (bas + tôt)."),
    (Charade, "Mon premier est un métal précieux. Mon second a des ailes et vit au ciel. Mon tout est un fruit.", "Orange (or + ange)."),
    (Charade, "Mon premier vaut cent. Mon second est une part sur trois. Mon tout est un petit chemin.", "Sentier (cent + tiers)."),
    (Charade, "Mon premier est le contraire de dur. Mon second est un gros poisson. Mon tout fait « bêê ».", "Mouton (mou + thon)."),
    (Charade, "Mon premier est le contraire de froid. Mon second est un chiffre. Mon tout se met au pied.", "Chaussette (chaud + sept)."),
    (Charade, "Mon premier miaule. Mon second est le contraire de tard. Mon tout est la demeure d'un roi.", "Château (chat + tôt)."),
    (Charade, "Mon premier est une boisson chaude. Mon second est le foyer de la cheminée. Mon tout présente des pièces.", "Théâtre (thé + âtre)."),
    (Charade, "Mon premier est une céréale qu'on mange beaucoup en Asie. Mon second coule du robinet. Mon tout se tire devant la fenêtre.", "Rideau (riz + eau)."),
    (Charade, "Mon premier se boit au biberon. Mon second est le verbe « tuer » au présent. Mon tout est une salade.", "Laitue (lait + tue)."),
    // Logique
    (Logique, "Combien de mois de l'année ont 28 jours ?", "Les 12 mois : tous ont au moins 28 jours."),
    (Logique, "Le père de Marie a quatre filles : Lili, Lala, Lulu… Comment s'appelle la quatrième ?", "Marie."),
    (Logique, "Pendant une course, tu doubles le deuxième. À quelle place es-tu ?", "Deuxième."),
    (Logique, "Pendant une course, tu doubles le dernier. À quelle place es-tu ?", "C'est impossible : personne n'est derrière le dernier."),
    (Logique, "Un coq pond un œuf pile sur le faîte d'un toit. De quel côté l'œuf tombe-t-il ?", "D'aucun côté : les coqs ne pondent pas."),
    (Logique, "Combien d'animaux de chaque espèce Moïse a-t-il fait monter dans l'arche ?", "Aucun : c'est Noé qui a construit l'arche."),
    (Logique, "Un avion s'écrase pile sur la frontière entre la France et l'Espagne. Où enterre-t-on les survivants ?", "Nulle part : on n'enterre pas les survivants."),
    (Logique, "Deux pères et deux fils vont à la pêche. Chacun attrape un poisson, et ils ne rapportent que trois poissons. Comment est-ce possible ?", "Ils ne sont que trois : un grand-père, son fils et son petit-fils."),
    (Logique, "Un fermier a 17 moutons. Tous meurent sauf 9. Combien lui en reste-t-il ?", "9."),
    (Logique, "Tu entres dans une pièce sombre avec une seule allumette. Il y a une bougie, une lampe à pétrole et un poêle. Qu'allumes-tu en premier ?", "L'allumette."),
    (Logique, "Dans chaque coin d'une pièce carrée se trouve un chat. Chaque chat voit trois chats. Combien y a-t-il de chats ?", "4."),
    (Logique, "Qui est le fils du père de ton frère, sans être ton frère ?", "Toi."),
    (Logique, "Un homme regarde un portrait et dit : « Je n'ai ni frère ni sœur, mais le père de cet homme est le fils de mon père. » Qui est sur le portrait ?", "Son fils."),
    (Logique, "Un train électrique roule vers le nord et le vent souffle vers l'est. Dans quelle direction part la fumée ?", "Nulle part : un train électrique ne fait pas de fumée."),
    (Logique, "Un médecin te donne trois comprimés : un toutes les demi-heures. Combien de temps dure le traitement ?", "Une heure (à 0, 30 et 60 minutes)."),
    (Logique, "Un tiroir contient des chaussettes noires et blanches mélangées. Dans le noir, combien dois-tu en prendre pour être sûr d'avoir une paire assortie ?", "3."),
    (Logique, "Deux mères et deux filles se partagent trois gâteaux, et chacune en a un entier. Comment ?", "Elles sont trois : une grand-mère, sa fille et sa petite-fille."),
    (Logique, "Trois boîtes sont étiquetées « pommes », « oranges » et « mélange », mais toutes les étiquettes sont fausses. En piochant un seul fruit, comment remettre les bonnes étiquettes ?", "On pioche dans « mélange » : elle ne contient qu'une sorte de fruit, on en déduit les deux autres."),
    (Logique, "Trois interrupteurs en bas commandent une ampoule à l'étage. Tu ne peux monter qu'une fois. Comment savoir lequel l'allume ?", "Allume le 1 longtemps, éteins-le, allume le 2 et monte : allumée = 2, éteinte et chaude = 1, éteinte et froide = 3."),
    (Logique, "Un fermier doit faire traverser une rivière à un loup, une chèvre et un chou, un seul à la fois. Le loup mange la chèvre, la chèvre mange le chou. Comment faire ?", "Chèvre, retour, loup, ramener la chèvre, chou, retour, chèvre."),
    (Logique, "Avec un sablier de 7 minutes et un de 4 minutes, comment mesurer exactement 9 minutes ?", "Lance les deux. À 4 min, retourne le petit. À 7 min, retourne le grand. À 8 min (fin du petit), retourne encore le grand : il lui reste 1 minute, soit 9 min en tout."),
    (Logique, "Avec un seau de 3 litres et un seau de 5 litres, comment mesurer exactement 4 litres ?", "Remplis le 5 L, verse dans le 3 L (il reste 2 L). Vide le 3 L, verse-y les 2 L. Remplis le 5 L et complète le 3 L : il reste 4 L."),
    (Logique, "Parmi 9 billes identiques, une est plus lourde. Avec une balance à plateaux, combien de pesées suffisent pour la trouver ?", "2 : on compare deux groupes de 3, puis deux billes du groupe suspect."),
    (Logique, "Un kilo de plumes ou un kilo de plomb : lequel est le plus lourd ?", "Aucun : ils pèsent tous les deux un kilo."),
    (Logique, "Tu as trois pommes et tu en prends deux. Combien en as-tu ?", "Deux : celles que tu as prises."),
    (Logique, "Un œuf met 4 minutes à cuire. Combien de temps pour en cuire trois ?", "4 minutes, s'ils cuisent ensemble."),
    (Logique, "La mère de Paul a quatre enfants : Printemps, Été, Automne… Comment s'appelle le quatrième ?", "Paul."),
    (Logique, "Si tu es dans une course et que tu dépasses la personne en deuxième position, puis qu'on te dépasse, à quelle place es-tu ?", "Troisième."),
    (Logique, "Quel est le prochain élément ? J, F, M, A, M, J, …", "J : les initiales des mois (juillet)."),
    (Logique, "Quel est le prochain élément ? L, M, M, J, V, …", "S : les initiales des jours (samedi)."),
    (Logique, "Quel est le prochain élément ? U, D, T, Q, C, …", "S : un, deux, trois, quatre, cinq, six."),
    // Calcul
    (Calcul, "Une raquette et une balle coûtent 1,10 € ensemble. La raquette coûte 1 € de plus que la balle. Combien coûte la balle ?", "5 centimes (et la raquette 1,05 €)."),
    (Calcul, "Des nénuphars doublent de surface chaque jour et couvrent tout le lac en 48 jours. En combien de jours en couvrent-ils la moitié ?", "47 jours."),
    (Calcul, "Combien de fois peut-on soustraire 10 de 100 ?", "Une seule fois : ensuite, on soustrait 10 de 90."),
    (Calcul, "5 machines fabriquent 5 pièces en 5 minutes. Combien de temps faut-il à 100 machines pour fabriquer 100 pièces ?", "5 minutes."),
    (Calcul, "3 chats attrapent 3 souris en 3 minutes. Combien de chats faut-il pour attraper 100 souris en 100 minutes ?", "3 chats."),
    (Calcul, "Un escargot au fond d'un puits de 10 mètres monte de 3 mètres le jour et glisse de 2 mètres la nuit. En combien de jours sort-il ?", "8 jours : à la fin du 7e jour il est à 7 m, le 8e il atteint 10 m."),
    (Calcul, "Combien de fois le chiffre 9 apparaît-il quand on écrit les nombres de 1 à 100 ?", "20 fois."),
    (Calcul, "Quel est le nombre suivant ? 1, 4, 9, 16, …", "25 : les carrés de 1, 2, 3, 4, 5."),
    (Calcul, "Quel est le nombre suivant ? 2, 6, 12, 20, 30, …", "42 : n × (n + 1)."),
    (Calcul, "Quel est le nombre suivant ? 1, 1, 2, 3, 5, 8, …", "13 : chaque nombre est la somme des deux précédents."),
    (Calcul, "Quel est le nombre suivant ? 2, 3, 5, 7, 11, …", "13 : les nombres premiers."),
    (Calcul, "Divise 30 par un demi, puis ajoute 10. Combien obtiens-tu ?", "70 (30 ÷ 0,5 = 60)."),
    (Calcul, "Un livre et un marque-page coûtent 11 €. Le livre coûte 10 € de plus que le marque-page. Combien coûte le marque-page ?", "50 centimes (et le livre 10,50 €)."),
    (Calcul, "Deux pièces font 30 centimes. L'une n'est pas une pièce de 10 centimes. Lesquelles ?", "Une pièce de 20 c et une de 10 c : c'est l'autre qui n'est pas une pièce de 10."),
    (Calcul, "Combien de mois de l'année ont 31 jours ?", "7 : janvier, mars, mai, juillet, août, octobre, décembre."),
    (Calcul, "Dans deux ans, j'aurai le double de l'âge que j'avais il y a cinq ans. Quel âge ai-je ?", "12 ans (14 = 2 × 7)."),
    (Calcul, "Combien vaut la somme de tous les nombres de 1 à 100 ?", "5 050 (50 paires qui font 101)."),
    (Calcul, "Je suis un nombre : multiplie-moi par n'importe quel nombre, le résultat sera toujours le même. Qui suis-je ?", "Zéro."),
    (Calcul, "Combien de minutes s'écoulent entre 13 h 45 et 15 h 10 ?", "85 minutes."),
    (Calcul, "Comment faire 1 000 en additionnant uniquement des 8 ?", "888 + 88 + 8 + 8 + 8 = 1 000."),
    (Calcul, "Ajoute un seul trait pour rendre l'égalité juste : 5 + 5 + 5 = 550.", "Un trait sur le premier « + » en fait un 4 : 545 + 5 = 550."),
    (Calcul, "Un marchand achète un vélo 70 €, le revend 80 €, le rachète 90 € et le revend 100 €. Combien a-t-il gagné ?", "20 €."),
    (Calcul, "Combien un cube a-t-il de sommets et d'arêtes ?", "8 sommets et 12 arêtes."),
    (Calcul, "À 3 h 15, quel angle forment les aiguilles d'une horloge ?", "7,5 degrés : la petite aiguille a avancé d'un quart d'heure."),
    (Calcul, "Combien de carrés compte un échiquier de 8 × 8, en comptant les carrés de toutes les tailles ?", "204."),
    (Calcul, "Un nombre à deux chiffres vaut quatre fois la somme de ses chiffres, et ses chiffres se suivent. Quel est-il ?", "12 : 4 × (1 + 2) = 12."),
    (Calcul, "Combien de poignées de main échangent 10 personnes si chacune serre la main de toutes les autres une seule fois ?", "45."),
    (Calcul, "Une bouteille pleine d'eau pèse 1 kg. À moitié vide, elle pèse 600 g. Combien pèse la bouteille vide ?", "200 g."),
    (Calcul, "Si 1 = 5, 2 = 25, 3 = 125, 4 = 625, alors 5 = ?", "1 : puisque 1 = 5, alors 5 = 1."),
];

/// Les énigmes en anglais : les jeux de mots ne se traduisent pas, la liste est écrite à part.
/// Les charades anglaises assemblent des mots composés (car + pet = carpet).
#[rustfmt::skip]
const RIDDLES_EN: &[(Kind, &str, &str)] = &[
    (Devinette, "What has keys but can't open locks?", "A piano."),
    (Devinette, "What gets wetter the more it dries?", "A towel."),
    (Devinette, "What has hands but can't clap?", "A clock."),
    (Devinette, "What has a neck but no head?", "A bottle."),
    (Devinette, "What can you catch but not throw?", "A cold."),
    (Devinette, "What has many teeth but can't bite?", "A comb."),
    (Devinette, "What goes up but never comes down?", "Your age."),
    (Devinette, "The more you take, the more you leave behind. What are they?", "Footsteps."),
    (Devinette, "What has an eye but cannot see?", "A needle."),
    (Devinette, "What is full of holes but still holds water?", "A sponge."),
    (Devinette, "What can travel around the world while staying in a corner?", "A stamp."),
    (Devinette, "What has one head, one foot and four legs?", "A bed."),
    (Devinette, "What belongs to you, but other people use it more than you do?", "Your name."),
    (Devinette, "What has words but never speaks?", "A book."),
    (Devinette, "What runs but never walks, and has a mouth but never talks?", "A river."),
    (Devinette, "What can you break without touching it?", "A promise."),
    (Devinette, "What has a thumb and four fingers but is not alive?", "A glove."),
    (Devinette, "I'm tall when I'm young and short when I'm old. What am I?", "A candle."),
    (Devinette, "What comes once in a minute, twice in a moment, but never in a thousand years?", "The letter M."),
    (Devinette, "What building has the most stories?", "A library."),
    (Devinette, "Forward I am heavy, backward I am not. What am I?", "A ton (backward: not)."),
    (Devinette, "What kind of room has no doors or windows?", "A mushroom."),
    (Devinette, "What has to be broken before you can use it?", "An egg."),
    (Devinette, "What can fill a room but takes up no space?", "Light."),
    (Devinette, "Why did the scarecrow win an award?", "Because he was outstanding in his field."),
    (Devinette, "What has a head and a tail but no body?", "A coin."),
    (Charade, "My first drives you around, my second sleeps on your lap. My whole covers the floor.", "Carpet (car + pet)."),
    (Charade, "My first lights up the day, my second grows in the garden. My whole turns to follow my first.", "Sunflower (sun + flower)."),
    (Charade, "My first is spread on toast, my second buzzes around the kitchen. My whole flutters from flower to flower.", "Butterfly (butter + fly)."),
    (Charade, "My first is used for frying, my second is eaten at birthdays. My whole gets flipped on Sunday mornings.", "Pancake (pan + cake)."),
    (Charade, "My first is at the end of your leg, my second bounces. My whole is played by two teams of eleven.", "Football (foot + ball)."),
    (Charade, "My first falls from the clouds, my second shoots arrows. My whole appears after the storm.", "Rainbow (rain + bow)."),
    (Charade, "My first opens the door, my second is a plank of wood. My whole is under your fingers when you type.", "Keyboard (key + board)."),
    (Charade, "My first shines at night, my second swims. My whole lives on the seabed with five arms.", "Starfish (star + fish)."),
    (Charade, "My first hears, my second goes on a finger. My whole dangles from my first.", "Earring (ear + ring)."),
    (Charade, "My first falls in winter, my second is a grown-up boy. My whole melts in spring.", "Snowman (snow + man)."),
    (Logique, "A rooster lays an egg on the very top of a barn roof. Which way does it roll?", "Neither: roosters don't lay eggs."),
    (Logique, "If you have me, you want to share me. If you share me, you no longer have me. What am I?", "A secret."),
    (Logique, "Mary's father has five daughters: Nana, Nene, Nini and Nono. What is the fifth daughter's name?", "Mary."),
    (Logique, "How many animals of each kind did Moses take on the ark?", "None: it was Noah's ark."),
    (Logique, "A plane crashes right on the border between two countries. Where do they bury the survivors?", "Nowhere: you don't bury survivors."),
    (Logique, "There are three apples and you take away two. How many apples do you have?", "Two: the ones you took."),
    (Logique, "What can you hold in your right hand but never in your left hand?", "Your left elbow."),
    (Logique, "A man walked in the rain with no umbrella and no hat, yet not a single hair on his head got wet. How?", "He was bald."),
    (Logique, "Which is heavier: a kilo of feathers or a kilo of bricks?", "Neither: they both weigh a kilo."),
    (Logique, "You're running a race and you pass the person in second place. What place are you in now?", "Second."),
    (Logique, "What occurs twice in a week, once in a year, but never in a day?", "The letter E."),
    (Logique, "Some months have 31 days, others have 30. How many have 28?", "All twelve of them."),
    (Logique, "I am an odd number. Take away one letter and I become even. What number am I?", "Seven (s-even)."),
    (Logique, "What is always in front of you but can't be seen?", "The future."),
    (Logique, "A girl has as many brothers as sisters, but each brother has only half as many brothers as sisters. How many children in the family?", "Seven: four girls and three boys."),
    (Logique, "Two fathers and two sons go fishing. Each catches one fish, yet they bring home only three. How?", "There are only three of them: a grandfather, his son and his grandson."),
    (Calcul, "A bat and a ball cost $1.10 in total. The bat costs $1.00 more than the ball. How much is the ball?", "5 cents (the bat costs $1.05)."),
    (Calcul, "If 5 machines take 5 minutes to make 5 widgets, how long do 100 machines take to make 100 widgets?", "5 minutes."),
    (Calcul, "A patch of lily pads doubles in size every day. It covers the whole lake in 48 days. When did it cover half of it?", "On day 47."),
    (Calcul, "Divide 30 by one half and add 10. What do you get?", "70 (30 ÷ 0.5 = 60)."),
    (Calcul, "I'm a three-digit number. My tens digit is five more than my ones digit, and my hundreds digit is eight less than my tens digit. What am I?", "194."),
    (Calcul, "How many times can you subtract 10 from 100?", "Once: after that, it's 90."),
    (Calcul, "A farmer has 17 sheep. All but 9 run away. How many are left?", "9."),
    (Calcul, "What is half of two plus two?", "3 (half of two is 1, plus 2)."),
    (Calcul, "A clock strikes six in 5 seconds. How long does it take to strike twelve?", "11 seconds: 5 gaps take 5 s, 12 strikes have 11 gaps."),
    (Calcul, "Using only addition, how can you add eight 8s to get 1,000?", "888 + 88 + 8 + 8 + 8 = 1,000."),
    (Calcul, "A snail at the bottom of a 10 m well climbs 3 m each day and slips back 2 m each night. On which day does it get out?", "On day 8."),
    (Calcul, "When I was 6, my sister was half my age. Now I'm 70. How old is she?", "67."),
];

/// La liste de la langue en vigueur.
fn riddles() -> &'static [(Kind, &'static str, &'static str)] {
    if crate::i18n::en() { RIDDLES_EN } else { RIDDLES }
}

/// Ordre de passage des énigmes : mélange fixe, pour éviter les répétitions.
fn order(kind: Option<Kind>) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..riddles().len()).filter(|&i| kind.is_none_or(|k| riddles()[i].0 == k)).collect();
    indices.shuffle(&mut StdRng::seed_from_u64(2026));
    indices
}

/// Énigme d'un jour donné (indice dans `RIDDLES`).
fn of_day(date: NaiveDate, kind: Option<Kind>) -> usize {
    let order = order(kind);
    let day = date.signed_duration_since(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()).num_days();
    order[day.rem_euclid(order.len() as i64) as usize]
}

pub fn build(today: NaiveDate, kind: Option<Kind>, number: Option<usize>, answer: Answer) -> Result<Doc> {
    let index = match number {
        Some(n) if (1..=riddles().len()).contains(&n) => n - 1,
        Some(n) => {
            bail!("{}", tr!("énigme n° {n} inconnue (1 à {})", "riddle No. {n} unknown (1 to {})", riddles().len()))
        }
        None => of_day(today, kind),
    };
    let (riddle_kind, question, solution) = riddles()[index];

    let mut doc = Doc::new();
    doc.header(&tr!("Énigme · {}", "Riddle · {}", riddle_kind.label()));
    doc.feed(1);
    doc.text(question, Style::default().bold());
    doc.text(&tr!("n° {}", "No. {}", index + 1), Style::default().small().align(Align::Right));
    match answer {
        Answer::UpsideDown => {
            doc.feed(1);
            doc.text(&tr!("Réponse : {solution}", "Answer: {solution}"), Style::default().small().upside_down());
        }
        Answer::Below => {
            doc.feed(1);
            doc.hanging(crate::i18n::t("Réponse : ", "Answer: "), solution, Style::default().small());
        }
        Answer::NextDay => {
            let yesterday = today.pred_opt().unwrap_or(today);
            let (_, previous, previous_solution) = riddles()[of_day(yesterday, kind)];
            doc.feed(1);
            doc.text(&tr!("Hier : {previous}", "Yesterday: {previous}"), Style::default().small());
            doc.hanging(crate::i18n::t("Réponse : ", "Answer: "), previous_solution, Style::default().small().bold());
        }
        Answer::None => {}
    }
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_riddle_comes_before_any_repeat() {
        let start = NaiveDate::from_ymd_opt(2026, 10, 7).unwrap();
        let mut seen: Vec<usize> =
            (0..riddles().len() as i64).map(|d| of_day(start + chrono::Duration::days(d), None)).collect();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), riddles().len());
    }

    #[test]
    fn kind_filter_and_numbers() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 7).unwrap();
        assert_eq!(riddles()[of_day(day, Some(Charade))].0, Charade);
        assert!(build(day, None, Some(1), Answer::Below).is_ok());
        assert!(build(day, None, Some(riddles().len() + 1), Answer::Below).is_err());
    }

    #[test]
    fn answer_is_printed_upside_down_last_line_first() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 7).unwrap();
        let doc = build(day, None, Some(25), Answer::UpsideDown).unwrap();
        let flipped: Vec<&str> = doc
            .ops
            .iter()
            .filter_map(|op| match op {
                crate::doc::Op::Line { text, style } if style.upside_down => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(flipped.len() > 1);
        assert!(flipped.last().unwrap().starts_with("Réponse"));
    }

    #[test]
    fn english_list_has_every_kind_and_no_repeat() {
        crate::i18n::with_lang(crate::i18n::Lang::En, || {
            for kind in [Devinette, Charade, Logique, Calcul] {
                assert_eq!(riddles()[of_day(NaiveDate::from_ymd_opt(2026, 10, 7).unwrap(), Some(kind))].0, kind);
            }
            let doc = build(NaiveDate::from_ymd_opt(2026, 10, 7).unwrap(), None, Some(1), Answer::Below).unwrap();
            assert!(
                doc.ops
                    .iter()
                    .any(|op| matches!(op, crate::doc::Op::Line { text, .. } if text.contains("What has keys")))
            );
        });
    }
}
