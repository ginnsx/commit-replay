import { create } from "zustand";

interface SelectionState {
  selectedRefs: Set<string>;
  toggle: (sourceRef: string) => void;
  selectAll: (refs: string[]) => void;
  clear: () => void;
  isSelected: (sourceRef: string) => boolean;
}

export const useSelectionStore = create<SelectionState>((set, get) => ({
  selectedRefs: new Set(),
  toggle: (sourceRef) =>
    set((state) => {
      const next = new Set(state.selectedRefs);
      if (next.has(sourceRef)) {
        next.delete(sourceRef);
      } else {
        next.add(sourceRef);
      }
      return { selectedRefs: next };
    }),
  selectAll: (refs) => set({ selectedRefs: new Set(refs) }),
  clear: () => set({ selectedRefs: new Set() }),
  isSelected: (sourceRef) => get().selectedRefs.has(sourceRef),
}));
