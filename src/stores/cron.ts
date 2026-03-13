/**
 * Cron State Store
 * Manages scheduled task state using Tauri IPC commands
 */
import { create } from 'zustand';
import { invokeIpc } from '@/lib/api-client';
import type { CronJob, CronJobCreateInput, CronJobUpdateInput } from '../types/cron';

interface CronState {
  jobs: CronJob[];
  loading: boolean;
  error: string | null;

  // Actions
  fetchJobs: () => Promise<void>;
  createJob: (input: CronJobCreateInput) => Promise<CronJob>;
  updateJob: (id: string, input: CronJobUpdateInput) => Promise<void>;
  deleteJob: (id: string) => Promise<void>;
  toggleJob: (id: string, enabled: boolean) => Promise<void>;
  triggerJob: (id: string) => Promise<void>;
  setJobs: (jobs: CronJob[]) => void;
}

export const useCronStore = create<CronState>((set, get) => ({
  jobs: [],
  loading: false,
  error: null,

  fetchJobs: async () => {
    set({ loading: true, error: null });

    try {
      const jobs = await invokeIpc<CronJob[]>('cron:list');
      set({ jobs: jobs || [], loading: false });
    } catch (error) {
      console.error('Failed to fetch cron jobs:', error);
      set({ error: String(error), loading: false });
    }
  },

  createJob: async (input) => {
    try {
      const job = await invokeIpc<CronJob>('cron:create', { input });
      set((state) => ({ jobs: [...state.jobs, job] }));
      return job;
    } catch (error) {
      console.error('Failed to create cron job:', error);
      throw error;
    }
  },

  updateJob: async (id, input) => {
    try {
      const updatedJob = await invokeIpc<CronJob>('cron:update', { id, input });
      set((state) => ({
        jobs: state.jobs.map((job) =>
          job.id === id ? updatedJob : job
        ),
      }));
    } catch (error) {
      console.error('Failed to update cron job:', error);
      throw error;
    }
  },

  deleteJob: async (id) => {
    try {
      await invokeIpc('cron:delete', { id });
      set((state) => ({
        jobs: state.jobs.filter((job) => job.id !== id),
      }));
    } catch (error) {
      console.error('Failed to delete cron job:', error);
      throw error;
    }
  },

  toggleJob: async (id, enabled) => {
    try {
      const updatedJob = await invokeIpc<CronJob>('cron:toggle', { id, enabled });
      set((state) => ({
        jobs: state.jobs.map((job) =>
          job.id === id ? updatedJob : job
        ),
      }));
    } catch (error) {
      console.error('Failed to toggle cron job:', error);
      throw error;
    }
  },

  triggerJob: async (id) => {
    try {
      await invokeIpc('cron:trigger', { id });
      // Refresh jobs after trigger to update lastRun/nextRun state
      await get().fetchJobs();
    } catch (error) {
      console.error('Failed to trigger cron job:', error);
      throw error;
    }
  },

  setJobs: (jobs) => set({ jobs }),
}));