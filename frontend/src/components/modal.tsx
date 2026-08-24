import type { ReactNode } from "react";
import { Modal, ModalOverlay, Dialog, Heading } from "react-aria-components";

import styles from "./modal.module.css";

export interface ClayModalProps {
  title: string;
  open: boolean;
  onClose: () => void;
  children: ReactNode;
}

/**
 * Catalog `modal` kind: `role="dialog"`, `aria-modal`, focus trap +
 * restoration and Escape handling come from React Aria; the scrim projects
 * the native `paint_scrim` contract.
 */
export function ClayModal({ title, open, onClose, children }: ClayModalProps) {
  return (
    <ModalOverlay
      isOpen={open}
      isDismissable
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
      className={styles.scrim}
    >
      <Modal>
        <Dialog className={styles.dialog} aria-label={title}>
          <Heading slot="title" className={styles.title}>
            {title}
          </Heading>
          {children}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
