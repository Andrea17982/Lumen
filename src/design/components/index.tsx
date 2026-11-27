// Design-system primitives: React Aria behaviour and accessibility, Lumen tokens for the look.
import type { ReactNode } from 'react';
import {
  Button as AriaButton,
  Checkbox,
  CheckboxGroup,
  ComboBox,
  Dialog as AriaDialog,
  DialogTrigger,
  FieldError,
  Heading,
  Input,
  Label,
  ListBox,
  ListBoxItem,
  Menu as AriaMenu,
  MenuItem as AriaMenuItem,
  MenuTrigger,
  Modal,
  ModalOverlay,
  Popover,
  Radio,
  RadioGroup,
  SearchField as AriaSearchField,
  Select as AriaSelect,
  SelectValue,
  Separator,
  Switch as AriaSwitch,
  Text,
  TextArea,
  TextField as AriaTextField,
  ToggleButton,
  ToggleButtonGroup,
  Tooltip as AriaTooltip,
  TooltipTrigger,
  type ButtonProps as AriaButtonProps,
  type Key,
} from 'react-aria-components';
import { Icon, type IconName } from '../icons';
import s from './controls.module.css';

export type Variant = 'primary' | 'secondary' | 'plain' | 'danger';

export function Button({ variant = 'secondary', icon, children, ...props }: AriaButtonProps & { variant?: Variant; icon?: IconName; children?: ReactNode }) {
  return (
    <AriaButton {...props} className={s.button} data-variant={variant}>
      {icon && <Icon name={icon} />}
      {children}
    </AriaButton>
  );
}

/** An icon-only button always carries a label for assistive technology and a tooltip. */
export function IconButton({ icon, label, variant = 'plain', ...props }: Omit<AriaButtonProps, 'children'> & { icon: IconName; label: string; variant?: Variant }) {
  return (
    <TooltipTrigger delay={600}>
      <AriaButton {...props} aria-label={label} className={s.button} data-variant={variant} data-icon-only="">
        <Icon name={icon} />
      </AriaButton>
      <AriaTooltip className={s.tooltip} offset={6}>
        {label}
      </AriaTooltip>
    </TooltipTrigger>
  );
}

interface FieldProps {
  label: string;
  description?: string;
  errorMessage?: string;
  placeholder?: string;
  value?: string;
  defaultValue?: string;
  onChange?: (value: string) => void;
  isDisabled?: boolean;
  isRequired?: boolean;
  isInvalid?: boolean;
  multiline?: boolean;
  type?: 'text' | 'password';
  autoComplete?: string;
}

export function TextField({ label, description, errorMessage, placeholder, multiline, ...props }: FieldProps) {
  return (
    <AriaTextField {...props} className={s.field}>
      <Label className={s.label}>{label}</Label>
      {multiline ? <TextArea className={s.input} placeholder={placeholder} rows={3} /> : <Input className={s.input} placeholder={placeholder} />}
      {description && (
        <Text slot="description" className={s.description}>
          {description}
        </Text>
      )}
      <FieldError className={s.error}>{errorMessage}</FieldError>
    </AriaTextField>
  );
}

export function SearchField({ label, placeholder, clearLabel, value, onChange }: { label: string; placeholder?: string; clearLabel: string; value?: string; onChange?: (v: string) => void }) {
  return (
    <AriaSearchField aria-label={label} value={value} onChange={onChange} className={s.search}>
      <Icon name="search" size={14} />
      <Input placeholder={placeholder ?? label} />
      <AriaButton className={s.clear} aria-label={clearLabel}>
        <Icon name="close" size={12} />
      </AriaButton>
    </AriaSearchField>
  );
}

export function Switch({ children, ...props }: { children: ReactNode; isSelected?: boolean; defaultSelected?: boolean; onChange?: (v: boolean) => void; isDisabled?: boolean }) {
  return (
    <AriaSwitch {...props} className={s.switch}>
      <span className={s.track}>
        <span className={s.thumb} />
      </span>
      {children}
    </AriaSwitch>
  );
}

/** Single choice among a few options, always one selected. */
export function Segmented<T extends string>({ label, value, options, onChange }: { label: string; value: T; options: { id: T; label: string }[]; onChange: (v: T) => void }) {
  return (
    <ToggleButtonGroup
      aria-label={label}
      selectionMode="single"
      disallowEmptySelection
      selectedKeys={[value]}
      onSelectionChange={(keys) => {
        const [next] = keys as Set<Key>;
        if (next !== undefined) onChange(next as T);
      }}
      className={s.segmented}
    >
      {options.map((o) => (
        <ToggleButton key={o.id} id={o.id} className={s.segment}>
          {o.label}
        </ToggleButton>
      ))}
    </ToggleButtonGroup>
  );
}

/** `compact` hides the label visually (screen readers still get it) and drops the border, for toolbars. */
export function Select<T extends string>({ label, value, options, onChange, isDisabled, compact }: { label: string; value: T; options: { id: T; label: string }[]; onChange: (v: T) => void; isDisabled?: boolean; compact?: boolean }) {
  return (
    <AriaSelect selectedKey={value} onSelectionChange={(k) => k !== null && onChange(k as T)} isDisabled={isDisabled} className={s.field}>
      <Label className={compact ? s.visuallyHidden : s.label}>{label}</Label>
      <AriaButton className={`${s.button} ${compact ? s.compactSelect : s.selectButton}`} data-variant={compact ? 'plain' : 'secondary'}>
        <SelectValue />
        <Icon name="chevronDown" size={14} />
      </AriaButton>
      <Popover className={s.popover} offset={4}>
        <ListBox className={s.list}>
          {options.map((o) => (
            <ListBoxItem key={o.id} id={o.id} textValue={o.label} className={s.item}>
              {({ isSelected }) => (
                <>
                  {o.label}
                  {isSelected && (
                    <span className={s.check}>
                      <Icon name="check" size={14} />
                    </span>
                  )}
                </>
              )}
            </ListBoxItem>
          ))}
        </ListBox>
      </Popover>
    </AriaSelect>
  );
}

export function ModelSelect({ label, value, options, onChange }: { label: string; value: string; options: { id: string; label: string }[]; onChange: (v: string) => void }) {
  return (
    <ComboBox aria-label={label} selectedKey={value} onSelectionChange={(key) => key !== null && onChange(String(key))} defaultItems={options} defaultFilter={(text, query) => text.toLocaleLowerCase().includes(query.toLocaleLowerCase())} className={s.modelSelect} menuTrigger="focus">
      <Input className={s.modelInput} />
      <AriaButton className={`${s.button} ${s.modelButton}`} data-variant="plain" aria-label={label}>
        <Icon name="chevronDown" size={14} />
      </AriaButton>
      <Popover className={s.popover} placement="top start" offset={4}>
        <ListBox<{ id: string; label: string }> className={s.list} renderEmptyState={() => '—'}>
          {(option) => (
            <ListBoxItem id={option.id} textValue={option.label} className={s.item}>
              {({ isSelected }) => (
                <>
                  {option.label}
                  {isSelected && (
                    <span className={s.check}>
                      <Icon name="check" size={14} />
                    </span>
                  )}
                </>
              )}
            </ListBoxItem>
          )}
        </ListBox>
      </Popover>
    </ComboBox>
  );
}

export interface MenuEntry {
  id: string;
  label: string;
  icon?: IconName;
  danger?: boolean;
  separatorBefore?: boolean;
  isDisabled?: boolean;
}

export function Menu({ label, icon = 'more', items, onAction }: { label: string; icon?: IconName; items: MenuEntry[]; onAction: (id: string) => void }) {
  return (
    <MenuTrigger>
      <AriaButton aria-label={label} className={s.button} data-variant="plain" data-icon-only="">
        <Icon name={icon} />
      </AriaButton>
      <Popover className={s.popover} placement="bottom end" offset={4}>
        <AriaMenu className={s.list} onAction={(k) => onAction(String(k))} disabledKeys={items.filter((i) => i.isDisabled).map((i) => i.id)}>
          {items.flatMap((i) => [
            ...(i.separatorBefore ? [<Separator key={`${i.id}-sep`} className={s.separator} />] : []),
            <AriaMenuItem key={i.id} id={i.id} textValue={i.label} className={s.item} data-danger={i.danger || undefined}>
              {i.icon && <Icon name={i.icon} />}
              {i.label}
            </AriaMenuItem>,
          ])}
        </AriaMenu>
      </Popover>
    </MenuTrigger>
  );
}

/**
 * Confirmation dialog: the trigger opens it, or the caller does with `isOpen` (for example once
 * what it describes has loaded). The confirm action runs and closes it.
 */
export function ConfirmDialog({
  trigger,
  isOpen,
  onOpenChange,
  title,
  body,
  confirmLabel,
  cancelLabel,
  danger,
  onConfirm,
}: {
  trigger?: ReactNode;
  isOpen?: boolean;
  onOpenChange?: (open: boolean) => void;
  title: string;
  body: ReactNode;
  confirmLabel: string;
  cancelLabel: string;
  danger?: boolean;
  onConfirm: () => void;
}) {
  const overlay = (
      <ModalOverlay className={s.overlay} isDismissable isOpen={isOpen} onOpenChange={onOpenChange}>
        <Modal className={s.modal}>
          <AriaDialog className={s.dialog} role="alertdialog">
            {({ close }) => (
              <>
                <Heading slot="title">{title}</Heading>
                {typeof body === 'string' ? <p>{body}</p> : body}
                <div className={s.dialogActions}>
                  <Button variant="secondary" onPress={close} autoFocus>
                    {cancelLabel}
                  </Button>
                  <Button
                    variant={danger ? 'danger' : 'primary'}
                    onPress={() => {
                      onConfirm();
                      close();
                    }}
                  >
                    {confirmLabel}
                  </Button>
                </div>
              </>
            )}
          </AriaDialog>
        </Modal>
      </ModalOverlay>
  );
  return trigger ? (
    <DialogTrigger>
      {trigger}
      {overlay}
    </DialogTrigger>
  ) : (
    overlay
  );
}

export type Tone = 'accent' | 'neutral' | 'success' | 'warning' | 'danger';
export function Badge({ tone = 'neutral', icon, children }: { tone?: Tone; icon?: IconName; children: ReactNode }) {
  return (
    <span className={s.badge} data-tone={tone}>
      {icon && <Icon name={icon} size={12} />}
      {children}
    </span>
  );
}

export interface Choice {
  id: string;
  label: string;
  description?: string;
}
/** One answer (radio buttons) or several (check boxes) from a short list, with descriptions. */
export function Choices({ label, choices, multiple, value, onChange, isDisabled }: { label: ReactNode; choices: Choice[]; multiple?: boolean; value: string[]; onChange: (v: string[]) => void; isDisabled?: boolean }) {
  const body = (
    <>
      <Label className={s.choicesLabel}>{label}</Label>
      {choices.map((c) => {
        const content = (
          <>
            <span className={s.mark} aria-hidden />
            <span className={s.choiceText}>
              <span>{c.label}</span>
              {c.description && <span className={s.description}>{c.description}</span>}
            </span>
          </>
        );
        return multiple ? (
          <Checkbox key={c.id} value={c.id} className={s.choice} data-multiple>
            {content}
          </Checkbox>
        ) : (
          <Radio key={c.id} value={c.id} className={s.choice}>
            {content}
          </Radio>
        );
      })}
    </>
  );
  return multiple ? (
    <CheckboxGroup className={s.choices} value={value} onChange={onChange} isDisabled={isDisabled}>
      {body}
    </CheckboxGroup>
  ) : (
    <RadioGroup className={s.choices} value={value[0] ?? null} onChange={(v) => onChange(v ? [v] : [])} isDisabled={isDisabled}>
      {body}
    </RadioGroup>
  );
}
