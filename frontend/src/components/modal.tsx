import type { ReactNode } from "react";
import {
  Modal,
  ModalOverlay,
  Dialog,
  Heading,
  Button,
} from "react-aria-components";

import { ClayIcon } from "./icon";
import styles from "./modal.module.css";
import { recipeAttributes } from "./recipe-attributes";

export interface ClayModalProps {
  title: string;
  open: boolean;
  onClose: () => void;
  children: ReactNode;
  /**
   * The content paints its own surface and head (the command palette is one
   * `commandCentre.default.root.rest` sheet, DESIGN.md §11): the dialog then
   * only positions and animates, and the title stays for assistive tech
   * without painting a second head above the content's own.
   */
  flush?: boolean;
  /** Trailing actions row: hairline-separated, cancel leading, primary last. */
  footer?: ReactNode;
}

/**
 * Catalog `modal` kind: `role="dialog"`, `aria-modal`, focus trap +
 * restoration and Escape handling come from React Aria; the scrim projects
 * the native `paint_scrim` contract.
 */
export function ClayModal({
  title,
  open,
  onClose,
  children,
  flush = false,
  footer,
}: ClayModalProps) {
  return (
    <ModalOverlay
      isOpen={open}
      isDismissable
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
      className={styles.scrim}
      {...recipeAttributes("modal", "scrim")}
    >
      <Modal>
        <Dialog
          className={flush ? `${styles.dialog} ${styles.flush}` : styles.dialog}
          aria-label={title}
          {...recipeAttributes("modal", "dialog")}
        >
          {!flush && (
            <Heading
              slot="title"
              className={styles.title}
              {...recipeAttributes("modal", "title")}
            >
              {title}
            </Heading>
          )}
          <Button
            slot="close"
            className={styles.close}
            aria-label="Close"
            onPress={onClose}
          >
            <ClayIcon name="action.close" />
          </Button>
          <div className={styles.body}>{children}</div>
          {footer && <div className={styles.foot}>{footer}</div>}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
