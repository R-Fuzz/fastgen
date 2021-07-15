#ifndef INTERFACE_H_
#define INTERFACE_H_
#include<deque>
#include<mutex>
#include<condition_variable>
#include "rgd.pb.h"

using namespace rgd;

void init(bool saving_whole, bool use_codecache);
void fini();

class TaskQueue 
{
  std::deque< std::pair<std::shared_ptr<SearchTask>, bool> > queue_;
  std::mutex mutex_;
  std::condition_variable condvar_;

  typedef std::lock_guard<std::mutex> lock;
  typedef std::unique_lock<std::mutex> ulock;

  public:
  void push(std::pair<std::shared_ptr<SearchTask>, bool> const &val)
  {
    lock l(mutex_); // prevents multiple pushes corrupting queue_
    bool wake = queue_.empty(); // we may need to wake consumer
    queue_.push_back(val);
    if (wake) condvar_.notify_one();
  }


  std::pair<std::shared_ptr<SearchTask>, bool>  pop()
  {
    ulock u(mutex_);
    while (queue_.empty())
      condvar_.wait(u);
    // now queue_ is non-empty and we still have the lock
    std::pair<std::shared_ptr<SearchTask>, bool> retval = queue_.front();
    queue_.pop_front();
    return retval;
  }
};
#endif

