import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { PathMapping } from "../lib/types";

export interface SvnConnection {
  url: string;
  username: string;
  password: string;
  limit: number;
}

export interface TargetConfig {
  wcPath: string;
  mappings: PathMapping[];
}

interface ConnectionState {
  svn: SvnConnection;
  target: TargetConfig;
  setSvn: (patch: Partial<SvnConnection>) => void;
  setTarget: (patch: Partial<TargetConfig>) => void;
  setMapping: (index: number, patch: Partial<PathMapping>) => void;
  addMapping: () => void;
  removeMapping: (index: number) => void;
}

const defaultSvn: SvnConnection = {
  url: "",
  username: "",
  password: "",
  limit: 20,
};

const defaultTarget: TargetConfig = {
  wcPath: "",
  // When SVN URL already ends at trunk, diff paths are often /src/... not /trunk/src/...
  mappings: [
    { from: "/trunk", to: "." },
    { from: "/", to: "." },
  ],
};

export const useConnectionStore = create<ConnectionState>()(
  persist(
    (set) => ({
      svn: defaultSvn,
      target: defaultTarget,
      setSvn: (patch) =>
        set((state) => ({
          svn: { ...state.svn, ...patch },
        })),
      setTarget: (patch) =>
        set((state) => ({
          target: { ...state.target, ...patch },
        })),
      setMapping: (index, patch) =>
        set((state) => {
          const mappings = [...state.target.mappings];
          mappings[index] = { ...mappings[index], ...patch };
          return { target: { ...state.target, mappings } };
        }),
      addMapping: () =>
        set((state) => ({
          target: {
            ...state.target,
            mappings: [...state.target.mappings, { from: "", to: "" }],
          },
        })),
      removeMapping: (index) =>
        set((state) => ({
          target: {
            ...state.target,
            mappings: state.target.mappings.filter((_, i) => i !== index),
          },
        })),
    }),
    { name: "copy-diff-connection" },
  ),
);
