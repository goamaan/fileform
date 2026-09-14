import AppKit
import FileformDomain

extension WorkspaceModel {
    func prepareTrim(_ job: FileJob) async {
        guard job.trimEditable else { return }
        guard let mediaPreviews else { job.trimError = "The verified media pack is required to preview and trim this file."; return }
        do {
            let input = job.input
            let timeline = try await mediaPreviews.inspect(input)
            try Task.checkCancellation()
            guard job.input == input, jobs.contains(where: { $0 === job }), job.trimEditable else { return }
            job.mediaTimeline = timeline
            if job.trimDraft == nil {
                job.trimDraft = .init(interval: .init(start: .init(ticks: 0, timescale: 1), end: timeline.duration),
                    format: timeline.video == nil ? .wav : .mp4,
                    outputName: input.deletingPathExtension().lastPathComponent + "-trimmed",
                    audioStream: timeline.audioTracks.first?.ordinal)
            }
            await refreshTrimPlan(job)
        } catch is CancellationError { }
        catch { job.trimError = error.localizedDescription }
    }

    func trimRequest(for job: FileJob) throws -> TransformationRequest {
        guard let draft = job.trimDraft else { throw FileformError(.invalidRequest, "Choose a trim range first.") }
        return try draft.request(input: job.input, folder: outputFolder ?? job.input.deletingLastPathComponent(), duration: job.mediaTimeline?.duration)
    }

    func refreshTrimPlan(_ job: FileJob) async {
        guard job.trimEditable, job.trimDraft != nil else { return }
        let draft = job.trimDraft, input = job.input, folder = outputFolder
        job.trimPlan = nil; job.trimError = nil
        do {
            let plan = try await engine.plan(trimRequest(for: job))
            try Task.checkCancellation()
            guard job.trimDraft == draft, job.input == input, outputFolder == folder, job.trimEditable else { return }
            job.trimPlan = plan
        } catch is CancellationError { }
        catch {
            guard job.trimDraft == draft, job.input == input, outputFolder == folder, job.trimEditable else { return }
            job.trimError = error.localizedDescription
        }
    }

    func changeTrim(_ job: FileJob, to value: MediaTrimDraft, undo: UndoManager? = nil, persist: Bool = true) {
        guard job.trimEditable, let prior = job.trimDraft, prior != value else { return }
        undo?.registerUndo(withTarget: self) { model in model.changeTrim(job, to: prior, undo: undo) }
        undo?.setActionName("Change trim")
        job.trimDraft = value; job.trimPlan = nil; job.trimError = nil
        if persist { saveSession() }
    }

    func finishTrimGesture(_ job: FileJob, prior: MediaTrimDraft, undo: UndoManager?) {
        guard prior != job.trimDraft else { return }
        undo?.registerUndo(withTarget: self) { model in model.changeTrim(job, to: prior, undo: undo) }
        undo?.setActionName("Adjust trim range")
        saveSession()
    }
}
