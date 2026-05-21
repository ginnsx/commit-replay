import { create } from "zustand";
import { persist } from "zustand/middleware";

export interface SvnConnection {
  url: string;
  username: string;
  password: string;
  limit: number;
}

interface ConnectionState {
  svn: SvnConnection;
  setSvn: (patch: Partial<SvnConnection>) => void;
}

const defaultSvn: SvnConnection = {
  url: "",
  username: "",
  password: "",
  limit: 20,
};

export const useConnectionStore = create<ConnectionState>()(
  persist(
    (set) => ({
      svn: defaultSvn,
      setSvn: (patch) =>
        set((state) => ({
          svn: { ...state.svn, ...patch },
        })),
    }),
    { name: "copy-diff-connection" },
  ),
);
