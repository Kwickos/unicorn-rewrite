import { Radio } from '@base-ui/react/radio'
import { RadioGroup } from '@base-ui/react/radio-group'
import { Check } from 'lucide-react'
import { cn } from 'cn'
import type { ProfileId } from '@/lib/desktop'
import { useT } from '@/lib/i18n'
import { PROFILES } from '@/lib/status'

type Props = {
  value: ProfileId
  customInstruction?: string
  onChange: (profile: ProfileId) => void
}

/**
 * Le profil utilisé par le raccourci. Un groupe de boutons radio : les
 * flèches passent d'un profil à l'autre, Tab en sort.
 */
export function ProfileList({ value, onChange }: Props) {
  const t = useT()

  return (
    <RadioGroup
      value={value}
      onValueChange={(next) => onChange(next as ProfileId)}
      aria-label={t('profiles')}
      className="flex flex-col"
    >
      {PROFILES.map((profile) => {
        return (
          <Radio.Root
            key={profile.id}
            value={profile.id}
            className={cn(
              'group flex h-8 cursor-default items-center gap-2 rounded-lg px-2 text-left outline-none',
              'transition-colors duration-[var(--duration-quick)] ease-[var(--ease-smooth-out)]',
              'hover:bg-muted/60 data-checked:bg-muted',
              'focus-visible:ring-2 focus-visible:ring-ring/60',
            )}
          >
            <span className="min-w-0 flex-1 truncate text-sm">{t(profile.label)}</span>
            <Radio.Indicator
              keepMounted
              className={cn(
                'flex size-4 shrink-0 items-center justify-center',
                'transition-[opacity,scale] duration-[var(--duration-quick)] ease-[var(--ease-smooth-out)]',
                'data-unchecked:scale-50 data-unchecked:opacity-0',
              )}
            >
              <Check className="size-3.5" />
            </Radio.Indicator>
          </Radio.Root>
        )
      })}
    </RadioGroup>
  )
}
