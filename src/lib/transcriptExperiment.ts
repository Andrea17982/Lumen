// Release experiments are compiled only with this explicit build-time flag. Ordinary
// builds ignore query parameters and Rollup removes the virtual-module import branch.
const experimental = import.meta.env.VITE_TRANSCRIPT_EXPERIMENT === '1';
const mode = experimental ? new URLSearchParams(location.search).get('transcriptExperiment') : null;
export const virtualTranscriptExperiment = experimental && mode === 'virtual';
export const plainParagraphEnabled = !experimental || mode === 'plain';
