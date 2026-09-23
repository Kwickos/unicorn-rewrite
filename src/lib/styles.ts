/**
 * Les quelques formes de bouton de l'app, partagées pour que le panneau, les
 * réglages et l'accueil gardent exactement le même geste.
 */

const transition =
  'transition-[background-color,color,opacity,scale] duration-[var(--duration-quick)] ease-[var(--ease-smooth-out)]'

const focus =
  'outline-none focus-visible:ring-2 focus-visible:ring-ring/60 focus-visible:ring-offset-0'

/** Ligne cliquable pleine largeur (pied de panneau, « Quitter »). */
export const rowButton = `flex h-8 items-center gap-2 rounded-lg px-2 text-sm text-muted-foreground hover:bg-muted hover:text-foreground ${transition} ${focus}`

/** Bouton carré à icône seule. */
export const iconButton = `flex size-8 shrink-0 items-center justify-center rounded-lg text-muted-foreground hover:bg-muted hover:text-foreground ${transition} ${focus}`

/** Petite action dans une ligne : la principale est pleine, les autres discrètes. */
export const primaryAction = `inline-flex h-7 shrink-0 items-center gap-1.5 rounded-md bg-foreground px-2.5 text-xs font-medium text-background hover:opacity-90 active:scale-[0.96] disabled:opacity-50 ${transition} ${focus}`

export const secondaryAction = `inline-flex h-7 shrink-0 items-center gap-1.5 rounded-md bg-muted px-2.5 text-xs font-medium text-foreground hover:bg-muted/70 active:scale-[0.96] disabled:opacity-50 ${transition} ${focus}`

export const quietAction = `inline-flex h-7 shrink-0 items-center gap-1.5 rounded-md px-2 text-xs text-muted-foreground hover:bg-muted hover:text-foreground active:scale-[0.96] ${transition} ${focus}`

export const fieldClass = `w-full rounded-md bg-muted px-2.5 py-1.5 text-sm placeholder:text-muted-foreground/80 ${focus}`

export const kbdClass =
  'rounded-[0.3rem] bg-muted px-1 py-px font-sans text-[0.7rem] font-medium tracking-wide text-foreground tabular-nums'
