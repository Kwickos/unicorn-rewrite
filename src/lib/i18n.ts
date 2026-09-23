import { createContext, createElement, useCallback, useContext } from 'react'

export type Locale = 'fr' | 'en'

export const LOCALES: { value: Locale; label: string }[] = [
  { value: 'fr', label: 'Français' },
  { value: 'en', label: 'English' },
]

/** Langue du système, consultée une seule fois : au tout premier lancement. */
export function detectLocale(): Locale {
  if (typeof navigator === 'undefined') return 'en'
  const preferred = navigator.languages?.length ? navigator.languages : [navigator.language]
  for (const tag of preferred) {
    if (tag?.toLowerCase().startsWith('fr')) return 'fr'
    if (tag?.toLowerCase().startsWith('en')) return 'en'
  }
  return 'en'
}

const MESSAGES = {
  fr: {
    // Profils
    profiles: 'Profils',
    profileCorrect: 'Corriger uniquement',
    profileCorrectHint: 'Orthographe, grammaire, ponctuation',
    profileNatural: 'Naturel',
    profileNaturalHint: 'Fluide et simple, proche de vous',
    profileProfessional: 'Professionnel',
    profileProfessionalHint: 'Clair, sans jargon inutile',
    profileWarm: 'Chaleureux',
    profileWarmHint: 'Cordial, sans familiarité forcée',
    profileConcise: 'Direct et concis',
    profileConciseHint: 'Plus court, rien d’utile en moins',
    profileCustom: 'Personnalisé',
    profileCustomEmpty: 'Consigne à écrire dans les réglages',

    // État
    ready: 'Prêt',
    readyHint: 'Sélectionnez un texte, puis',
    reading: 'Lecture de la sélection…',
    rewriting: 'Reformulation…',
    rewritingIn: 'Reformulation…',
    replacing: 'Remplacement…',
    done: 'Remplacé',
    doneIn: 'Remplacé',
    unchanged: 'Rien à changer',
    restored: 'Restauré',
    cancelled: 'Annulé',
    cancel: 'Annuler',
    cancelHint: 'Échap',
    restore: 'Restaurer',
    restoreHint: 'Remettre le texte d’origine',
    copyResult: 'Copier le résultat',
    copyOriginal: 'Copier l’original',
    copied: 'Copié',
    dismiss: 'Fermer',
    resultTitle: 'Non appliqué',
    mockNotice: 'Fournisseur simulé (développement)',

    // Erreurs
    errPermission: 'Accès Accessibilité requis.',
    errNoSelection: 'Aucun texte sélectionné.',
    errSecureField: 'Champ protégé.',
    errSelfTarget: 'Sélectionnez du texte dans une autre app.',
    errReadFailed: 'Sélection illisible dans cette app.',
    errTooLong: 'Sélection trop longue.',
    errMissingKey: 'Clé API manquante.',
    errInvalidKey: 'Clé API refusée.',
    errQuota: 'Quota atteint.',
    errNetwork: 'Réseau indisponible.',
    errTimeout: 'Délai dépassé.',
    errBlocked: 'Texte refusé par le service.',
    errTruncated: 'Réponse incomplète.',
    errEmpty: 'Réponse vide.',
    errUnusable: 'Réponse inutilisable.',
    errProvider: 'Erreur du service.',
    errTargetChanged: 'La sélection a changé : rien remplacé.',
    errReplaceFailed: 'Remplacement refusé par l’app.',
    errRestoreFailed: 'Restauration impossible.',
    actionAllow: 'Autoriser',
    actionAddKey: 'Ajouter une clé',

    // Pied
    settings: 'Réglages',
    closeSettings: 'Fermer les réglages',

    // Réglages
    shortcut: 'Raccourci',
    shortcutRecord: 'Modifier le raccourci',
    shortcutRecording: 'Tapez la combinaison…',
    shortcutRecordingHint: 'Échap pour annuler',
    shortcutInvalid: 'Combinaison illisible.',
    shortcutNeedsModifier: 'Ajoutez ⌃, ⌥ ou ⌘ : une touche seule sert à écrire.',
    shortcutTypesCharacter: '⌥ seul tape un caractère spécial. Ajoutez ⌃ ou ⌘.',
    shortcutAppShortcut: 'Déjà un raccourci courant des apps. Ajoutez ⌃ ou ⌥.',
    shortcutSystem: 'Déjà utilisé par macOS.',
    shortcutUnavailable: 'Indisponible. Essayez une autre combinaison.',
    shortcutSaved: 'Raccourci enregistré',
    customInstruction: 'Consigne personnalisée',
    customInstructionPlaceholder: 'Ex. : phrases courtes, ton posé, pas d’anglicismes',
    apiKey: 'Clé API',
    apiKeyStored: 'Enregistrée dans le Trousseau',
    apiKeyMissing: 'Aucune clé',
    apiKeyPlaceholder: 'Clé Cerebras, Qwen, Groq ou Gemini',
    apiKeySave: 'Enregistrer',
    apiKeyCheck: 'Vérifier',
    apiKeyReplace: 'Modifier',
    apiKeyDelete: 'Supprimer',
    apiKeyChecking: 'Vérification…',
    apiKeyValid: 'Clé valide',
    apiKeyUnverified: 'Enregistrée, mais Google n’a pas pu être joint',
    apiKeyGet: 'Obtenir une clé Cerebras (gratuite)',
    permission: 'Accessibilité',
    permissionGranted: 'Autorisée',
    permissionMissing: 'Non autorisée',
    permissionOpen: 'Ouvrir',
    launchAtLogin: 'Lancer au démarrage',
    appearance: 'Apparence',
    themeSystem: 'Auto',
    themeLight: 'Clair',
    themeDark: 'Sombre',
    language: 'Langue',
    model: 'Modèle',
    modelSearch: 'Rechercher',
    modelFast: 'Rapides',
    modelAnyPrice: 'Tous prix',
    modelNone: 'Aucun modèle avec ces filtres.',
    modelUpgraded: 'Nouvelle version (avant : {model})',
    loading: 'Chargement',
    updateReady: 'Mettre à jour ({version})',
    privacyShort: 'Chaque sélection est envoyée à Google pour être reformulée. Rien n’est conservé par l’app.',
    quit: 'Quitter Unicorn Rewrite',
    modifierControl: 'Contrôle',
    modifierAlt: 'Option',
    modifierShift: 'Majuscule',
    modifierSuper: 'Commande',
    spaceKey: 'Espace',

    // Accueil
    welcomeTitle: 'Reformuler sans quitter votre app',
    welcomeIntro: 'Sélectionnez un texte n’importe où, appuyez sur {shortcut} : il est corrigé ou reformulé sur place, avec le profil choisi. Rien n’est jamais envoyé à votre place.',
    stepPermission: 'Autoriser l’Accessibilité',
    stepPermissionHint: 'macOS l’exige pour lire la sélection et la remplacer. Activez Unicorn Rewrite dans la liste qui s’ouvre.',
    stepPermissionRetry: 'Vérifier à nouveau',
    stepPermissionRestart: 'Toujours non détectée ? Quittez et relancez l’app.',
    stepKey: 'Clé API',
    stepKeyHint: 'Gratuite sur Google AI Studio. Elle reste dans le Trousseau de votre Mac.',
    stepPrivacy: 'Ce qui quitte votre Mac',
    privacyLong: 'Seul le texte sélectionné part chez Google (Gemini API), au moment où vous appuyez sur le raccourci. L’app ne garde aucun historique et ne journalise aucun texte. Avec une clé gratuite, Google peut utiliser ces textes pour améliorer ses produits ; activez la facturation pour l’éviter.',
    start: 'Commencer',
    done_: 'Terminé',
  },
  en: {
    profiles: 'Profiles',
    profileCorrect: 'Fix only',
    profileCorrectHint: 'Spelling, grammar, punctuation',
    profileNatural: 'Natural',
    profileNaturalHint: 'Fluent and simple, close to your voice',
    profileProfessional: 'Professional',
    profileProfessionalHint: 'Clear, no needless jargon',
    profileWarm: 'Warm',
    profileWarmHint: 'Friendly, never forced',
    profileConcise: 'Direct and concise',
    profileConciseHint: 'Shorter, nothing useful lost',
    profileCustom: 'Custom',
    profileCustomEmpty: 'Write the instruction in settings',

    ready: 'Ready',
    readyHint: 'Select some text, then press',
    reading: 'Reading the selection…',
    rewriting: 'Rewriting…',
    rewritingIn: 'Rewriting…',
    replacing: 'Replacing…',
    done: 'Replaced',
    doneIn: 'Replaced',
    unchanged: 'Nothing to change',
    restored: 'Restored',
    cancelled: 'Cancelled',
    cancel: 'Cancel',
    cancelHint: 'Esc',
    restore: 'Restore',
    restoreHint: 'Put the original text back',
    copyResult: 'Copy result',
    copyOriginal: 'Copy original',
    copied: 'Copied',
    dismiss: 'Close',
    resultTitle: 'Not applied',
    mockNotice: 'Simulated provider (development)',

    errPermission: 'Accessibility access required.',
    errNoSelection: 'No text selected.',
    errSecureField: 'Protected field.',
    errSelfTarget: 'Select text in another app.',
    errReadFailed: 'Can’t read the selection in this app.',
    errTooLong: 'Selection too long.',
    errMissingKey: 'API key missing.',
    errInvalidKey: 'API key rejected.',
    errQuota: 'Quota reached.',
    errNetwork: 'Network unavailable.',
    errTimeout: 'Timed out.',
    errBlocked: 'Text refused by the service.',
    errTruncated: 'Incomplete answer.',
    errEmpty: 'Empty answer.',
    errUnusable: 'Unusable answer.',
    errProvider: 'Service error.',
    errTargetChanged: 'Selection changed: nothing replaced.',
    errReplaceFailed: 'The app refused the replacement.',
    errRestoreFailed: 'Can’t restore.',
    actionAllow: 'Allow',
    actionAddKey: 'Add a key',

    settings: 'Settings',
    closeSettings: 'Close settings',

    shortcut: 'Shortcut',
    shortcutRecord: 'Change shortcut',
    shortcutRecording: 'Type the combination…',
    shortcutRecordingHint: 'Esc to cancel',
    shortcutInvalid: 'Unreadable combination.',
    shortcutNeedsModifier: 'Add ⌃, ⌥ or ⌘: a single key is for typing.',
    shortcutTypesCharacter: '⌥ alone types a special character. Add ⌃ or ⌘.',
    shortcutAppShortcut: 'Already a common app shortcut. Add ⌃ or ⌥.',
    shortcutSystem: 'Already used by macOS.',
    shortcutUnavailable: 'Unavailable. Try another combination.',
    shortcutSaved: 'Shortcut saved',
    customInstruction: 'Custom instruction',
    customInstructionPlaceholder: 'E.g. short sentences, calm tone, no jargon',
    apiKey: 'API key',
    apiKeyStored: 'Stored in the Keychain',
    apiKeyMissing: 'No key',
    apiKeyPlaceholder: 'Cerebras, Qwen, Groq or Gemini key',
    apiKeySave: 'Save',
    apiKeyCheck: 'Check',
    apiKeyReplace: 'Change',
    apiKeyDelete: 'Remove',
    apiKeyChecking: 'Checking…',
    apiKeyValid: 'Key is valid',
    apiKeyUnverified: 'Saved, but Google couldn’t be reached',
    apiKeyGet: 'Get a Cerebras key (free)',
    permission: 'Accessibility',
    permissionGranted: 'Allowed',
    permissionMissing: 'Not allowed',
    permissionOpen: 'Open',
    launchAtLogin: 'Launch at login',
    appearance: 'Appearance',
    themeSystem: 'Auto',
    themeLight: 'Light',
    themeDark: 'Dark',
    language: 'Language',
    model: 'Model',
    modelSearch: 'Search',
    modelFast: 'Fast',
    modelAnyPrice: 'Any price',
    modelNone: 'No model matches these filters.',
    modelUpgraded: 'New version (was {model})',
    loading: 'Loading',
    updateReady: 'Update ({version})',
    privacyShort: 'Each selection is sent to Google to be rewritten. The app keeps nothing.',
    quit: 'Quit Unicorn Rewrite',
    modifierControl: 'Control',
    modifierAlt: 'Option',
    modifierShift: 'Shift',
    modifierSuper: 'Command',
    spaceKey: 'Space',

    welcomeTitle: 'Rewrite without leaving your app',
    welcomeIntro: 'Select text anywhere and press {shortcut}: it’s fixed or rewritten in place, with the profile you picked. Nothing is ever sent on your behalf.',
    stepPermission: 'Allow Accessibility',
    stepPermissionHint: 'macOS requires it to read the selection and replace it. Turn on Unicorn Rewrite in the list that opens.',
    stepPermissionRetry: 'Check again',
    stepPermissionRestart: 'Still not detected? Quit and relaunch the app.',
    stepKey: 'API key',
    stepKeyHint: 'Free on Google AI Studio. It stays in your Mac’s Keychain.',
    stepPrivacy: 'What leaves your Mac',
    privacyLong: 'Only the selected text goes to Google (Gemini API), when you press the shortcut. The app keeps no history and logs no text. With a free key, Google may use these texts to improve its products; enable billing to avoid that.',
    start: 'Get started',
    done_: 'Done',
  },
} as const

export type MessageKey = keyof (typeof MESSAGES)['fr']

export function translate(
  locale: Locale,
  key: MessageKey,
  params?: Record<string, string | number>,
): string {
  const template: string = MESSAGES[locale][key] ?? MESSAGES.en[key]
  if (!params) return template
  return template.replace(/\{(\w+)\}/g, (match, name: string) =>
    name in params ? String(params[name]) : match,
  )
}

const LocaleContext = createContext<Locale>('en')

export function LocaleProvider({
  locale,
  children,
}: {
  locale: Locale
  children: React.ReactNode
}) {
  return createElement(LocaleContext, { value: locale }, children)
}

export function useLocale(): Locale {
  return useContext(LocaleContext)
}

/** `t('doneIn', { app })` — interpolation simple par accolades. */
export function useT() {
  const locale = useLocale()
  return useCallback(
    (key: MessageKey, params?: Record<string, string | number>) => translate(locale, key, params),
    [locale],
  )
}
