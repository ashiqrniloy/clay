import type { ReactNode } from "react";
import {
  Modal,
  ModalOverlay,
  Dialog,
  Heading,
  Button,
} from "react-aria-components";

import { ClayText } from "./text";

import styles from "./modal.module.css";
import { recipeAttributes } from "./recipe-attributes";

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
      {...recipeAttributes("modal", "scrim")}
    >
      <Modal>
        <Dialog
          className={styles.dialog}
          aria-label={title}
          {...recipeAttributes("modal", "dialog")}
        >
          <Heading
            slot="title"
            className={styles.title}
            {...recipeAttributes("modal", "title")}
          >
            {title}
          </Heading>
          <Button
            slot="close"
            className={styles.close}
            aria-label="Close"
            onPress={onClose}
          >
            <ClayText aria-hidden="true">×</ClayText>
          </Button>
          {children}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
