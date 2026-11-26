#include "browser_core/agent_close_rule.h"

namespace browser_core {

AgentCloseAsk::Step AgentCloseAsk::Answering(Answer answer) {
  if (answered_) {
    return {};
  }
  answered_ = true;
  return {.answer = answer};
}

AgentCloseAsk::Step AgentCloseAsk::PageAnswered(bool proceed) {
  if (answered_ || agreed_) {
    return {};
  }
  if (!proceed) {
    return Answering(Answer::kAsks);
  }
  agreed_ = true;
  return {};
}

AgentCloseAsk::Step AgentCloseAsk::Navigated() {
  // Once the page agreed, Chromium is closing it, and its going answers.
  if (agreed_) {
    return {};
  }
  return Answering(Answer::kStays);
}

AgentCloseAsk::Step AgentCloseAsk::RendererGone() {
  return {.close = !answered_};
}

AgentCloseAsk::Step AgentCloseAsk::TimedOut() {
  return {.close = !answered_};
}

AgentCloseAsk::Step AgentCloseAsk::Closing(bool users) {
  if (answered_) {
    return {};
  }
  if (users && !agreed_) {
    return Answering(Answer::kStays);
  }
  return {.close = true};
}

AgentCloseAsk::Step AgentCloseAsk::StillThere() {
  return Answering(Answer::kStays);
}

AgentCloseAsk::Step AgentCloseAsk::PageGone(bool tab_stays) {
  return Answering(tab_stays ? Answer::kStays : Answer::kClosed);
}

AgentArchive::Does AgentArchive::What() const {
  if (!handed || held) {
    return Does::kRefused;
  }
  if (pinned) {
    return Does::kPinned;
  }
  if (asks && !discard) {
    return Does::kAsks;
  }
  return Does::kArchives;
}

}  // namespace browser_core
